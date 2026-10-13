use std::collections::HashMap;

use vb6core::types::{TypeInfo, VBType};
use vb6parse::files::ModuleFile;
use vb6parse::io::SourceFile;
use vb6pcode::canonical::{
    self, Constant, ConstantValue, Instruction, ModuleVariable, Opcode, Operand, ProcedureCode,
    ProcedureId, Program, Sentinel, SideEffect, SlotKind, SourceMapEntry, StackEffect, StackValue,
    SymbolRef, VB6R_FORMAT_VERSION, VariableRef, VariableSlot,
};
use vb6semantic::{
    ContinueTarget, Declaration, DeclarationKind, Expression, ExpressionKind, HirAnalysis,
    LoopKind, NodeInfo, Operator, Parameter, PassingMode, Procedure, Statement, StatementKind,
    TypedProgram,
};

/// A source-located compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileDiagnostic {
    /// Human-readable reason compilation stopped.
    pub message: String,
    /// Source location when available.
    pub location: Option<vb6semantic::SourceLocation>,
}

/// Compile a parsed and semantically analyzed HIR module to validated VB6R P-Code.
pub fn compile_hir(analysis: &HirAnalysis) -> Result<Program, Vec<CompileDiagnostic>> {
    let Some(program) = analysis.program.as_ref() else {
        return Err(analysis
            .errors
            .iter()
            .map(|error| CompileDiagnostic {
                message: error.to_string(),
                location: None,
            })
            .collect());
    };
    if !program.compilable || !analysis.errors.is_empty() {
        return Err(vec![CompileDiagnostic {
            message: "semantic analysis did not produce compilable HIR".to_string(),
            location: Some(program.info.location.clone()),
        }]);
    }
    PCodeBuilder::new().build(program)
}

/// Parse, analyze, lower, and validate one VB6 source module.
pub fn compile_source(name: &str, source: &str) -> Result<Program, Vec<CompileDiagnostic>> {
    let file = SourceFile::from_string(name, source);
    let (module, parse_errors) = ModuleFile::parse(&file).unpack();
    if !parse_errors.is_empty() {
        return Err(parse_errors
            .into_iter()
            .map(|error| CompileDiagnostic {
                message: format!("{error:?}"),
                location: None,
            })
            .collect());
    }
    let Some(module) = module else {
        return Err(vec![CompileDiagnostic {
            message: "parser produced no module".to_string(),
            location: None,
        }]);
    };
    let analysis = vb6semantic::SemanticAnalyzer::new()
        .analyze_module_hir(&module)
        .map_err(|error| {
            vec![CompileDiagnostic {
                message: error.to_string(),
                location: None,
            }]
        })?;
    compile_hir(&analysis)
}

/// Builder for module-wide tables and canonical P-Code procedures.
#[derive(Debug, Default)]
pub struct PCodeBuilder;

impl PCodeBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self
    }

    /// Lower the supported HIR subset and validate the resulting program.
    pub fn build(&self, typed: &TypedProgram) -> Result<Program, Vec<CompileDiagnostic>> {
        let mut globals = Vec::new();
        let mut global_ids = HashMap::new();
        for declaration in &typed.declarations {
            match module_variable(declaration, globals.len()) {
                Ok(variable) => {
                    global_ids.insert(variable.symbol.clone(), variable.index);
                    globals.push(variable);
                }
                Err(error) => return Err(vec![error]),
            }
        }
        let proc_ids: HashMap<_, _> = typed
            .procedures
            .iter()
            .enumerate()
            .map(|(index, procedure)| {
                (
                    procedure.name.to_ascii_lowercase(),
                    ProcedureId(index as u32),
                )
            })
            .collect();
        let signatures: HashMap<_, _> = typed
            .procedures
            .iter()
            .map(|procedure| (procedure.name.to_ascii_lowercase(), procedure))
            .collect();
        let mut emitter = ModuleEmitter {
            constants: Vec::new(),
            globals,
            global_ids,
            proc_ids,
            signatures,
        };
        let mut procedures = Vec::with_capacity(typed.procedures.len());
        for procedure in &typed.procedures {
            procedures.push(emitter.compile_procedure(procedure)?);
        }
        let mut program = Program {
            format_version: VB6R_FORMAT_VERSION,
            module_name: typed.name.clone(),
            constants: emitter.constants,
            variables: emitter.globals,
            procedures,
        };
        canonical::validate(&mut program).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| CompileDiagnostic {
                    message: format!("generated P-Code failed validation: {error}"),
                    location: None,
                })
                .collect::<Vec<_>>()
        })?;
        Ok(program)
    }
}

struct ModuleEmitter<'a> {
    constants: Vec<Constant>,
    globals: Vec<ModuleVariable>,
    global_ids: HashMap<SymbolRef, u16>,
    proc_ids: HashMap<String, ProcedureId>,
    signatures: HashMap<String, &'a Procedure>,
}

impl ModuleEmitter<'_> {
    fn compile_procedure(
        &mut self,
        procedure: &Procedure,
    ) -> Result<ProcedureCode, Vec<CompileDiagnostic>> {
        let Some(symbol) = procedure.info.symbol.as_ref().map(symbol_ref) else {
            return Err(vec![diagnostic(
                "procedure has no bound semantic symbol",
                &procedure.info,
            )]);
        };
        let id = *self
            .proc_ids
            .get(&procedure.name.to_ascii_lowercase())
            .ok_or_else(|| vec![diagnostic("procedure id is missing", &procedure.info)])?;
        let mut parameters = Vec::new();
        let mut local_ids = HashMap::new();
        let mut local_slots = Vec::new();
        for parameter in &procedure.signature.parameters {
            let slot = variable_slot(
                parameter,
                parameters.len(),
                parameter_kind(parameter.passing),
            )?;
            local_ids.insert(slot.symbol.clone(), VariableRef::Local(slot.index));
            parameters.push(slot);
        }
        let mut return_slot = None;
        if let Some(return_type) = &procedure.signature.return_type {
            let slot = VariableSlot {
                index: parameters.len() as u16,
                name: procedure.name.clone(),
                ty: return_type.clone(),
                kind: SlotKind::Local,
                optional: false,
                symbol: symbol.clone(),
            };
            return_slot = Some(slot.index);
            local_ids.insert(slot.symbol.clone(), VariableRef::Local(slot.index));
            local_slots.push(slot);
        }
        collect_local_declarations(&procedure.body, &mut |declaration| {
            if let DeclarationKind::Variable { constant: false } = &declaration.kind {
                if let Some(symbol) = declaration.info.symbol.as_ref().map(symbol_ref) {
                    if local_ids.contains_key(&symbol) {
                        return Err(diagnostic(
                            "duplicate local symbol in HIR variable table",
                            &declaration.info,
                        ));
                    }
                    let slot_index = parameters.len() + local_slots.len();
                    let slot = variable_slot_from_declaration(declaration, slot_index)?;
                    local_ids.insert(symbol, VariableRef::Local(slot.index));
                    local_slots.push(slot);
                } else {
                    return Err(diagnostic(
                        "local variable has no bound semantic symbol",
                        &declaration.info,
                    ));
                }
            } else {
                return Err(diagnostic(
                    "local constants are not represented with initializer values in HIR",
                    &declaration.info,
                ));
            }
            Ok(())
        })?;
        let mut output = ProcedureCode {
            id,
            name: procedure.name.clone(),
            symbol,
            parameters,
            locals: local_slots,
            return_type: procedure.signature.return_type.as_ref().map(value_type),
            instructions: Vec::new(),
            source_map: Vec::new(),
            max_stack: 0,
        };
        let mut context = ProcedureEmitter {
            module: self,
            procedure: &mut output,
            local_ids,
            return_slot,
            labels: HashMap::new(),
            pending_branches: Vec::new(),
            next_label: 0,
            active_loop_targets: Vec::new(),
            next_hidden_loop: 0,
        };
        for statement in &procedure.body {
            context.compile_statement(statement)?;
        }
        if let Some(return_type) = context.procedure.return_type.clone() {
            let slot = context.return_slot.ok_or_else(|| {
                vec![diagnostic(
                    "Function result slot was not allocated",
                    &procedure.info,
                )]
            })?;
            context.emit(
                Opcode::LoadLocal,
                Operand::Variable(VariableRef::Local(slot)),
                StackEffect {
                    pops: vec![],
                    pushes: vec![stack_value(&return_type, false)],
                },
                vec![VariableRef::Local(slot)],
                vec![SideEffect::ReadVariable],
                &procedure.info,
            );
            context.emit(
                Opcode::ReturnValue,
                Operand::None,
                StackEffect {
                    pops: vec![stack_value(&return_type, false)],
                    pushes: vec![],
                },
                Vec::new(),
                vec![SideEffect::ControlFlow],
                &procedure.info,
            );
        } else {
            context.emit(
                Opcode::ReturnVoid,
                Operand::None,
                StackEffect::default(),
                Vec::new(),
                vec![SideEffect::ControlFlow],
                &procedure.info,
            );
        }
        context.resolve_branches()?;
        Ok(output)
    }
}

struct ProcedureEmitter<'a, 'p> {
    module: &'a mut ModuleEmitter<'p>,
    procedure: &'a mut ProcedureCode,
    local_ids: HashMap<SymbolRef, VariableRef>,
    return_slot: Option<u16>,
    labels: HashMap<Label, u32>,
    pending_branches: Vec<(usize, Label)>,
    next_label: u32,
    active_loop_targets: Vec<(ContinueTarget, Label)>,
    next_hidden_loop: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Label(u32);

impl ProcedureEmitter<'_, '_> {
    fn compile_statement(&mut self, statement: &Statement) -> Result<(), Vec<CompileDiagnostic>> {
        match &statement.kind {
            StatementKind::Declaration(Declaration {
                kind: DeclarationKind::Variable { constant: false },
                ..
            }) => Ok(()),
            StatementKind::Declaration(declaration) => Err(vec![diagnostic(
                "constant declaration has no initializer value in HIR",
                &declaration.info,
            )]),
            StatementKind::Assignment { target, value } => {
                let target_name = match &target.kind {
                    ExpressionKind::Name(name) => name,
                    _ => {
                        return Err(vec![diagnostic(
                            "assignment target is not a variable name",
                            &target.info,
                        )]);
                    }
                };
                self.compile_expression(value)?;
                let target_ref = if target_name.eq_ignore_ascii_case(&self.procedure.name) {
                    self.return_slot.map(VariableRef::Local).ok_or_else(|| {
                        vec![diagnostic(
                            "assignment to procedure result outside a Function",
                            &target.info,
                        )]
                    })?
                } else {
                    self.resolve_variable(target).ok_or_else(|| {
                        vec![diagnostic(
                            "assignment target is not in the procedure or module variable table",
                            &target.info,
                        )]
                    })?
                };
                let ty = self.variable_type(target_ref).ok_or_else(|| {
                    vec![diagnostic(
                        "assignment target slot is missing",
                        &target.info,
                    )]
                })?;
                let (opcode, effects) = match target_ref {
                    VariableRef::Local(_) => (Opcode::StoreLocal, SideEffect::WriteVariable),
                    VariableRef::Module(_) => (Opcode::StoreModule, SideEffect::WriteVariable),
                };
                self.emit(
                    opcode,
                    Operand::Variable(target_ref),
                    StackEffect {
                        pops: vec![stack_value(&ty, false)],
                        pushes: vec![],
                    },
                    vec![target_ref],
                    vec![effects],
                    &statement.info,
                );
                Ok(())
            }
            StatementKind::Expression(expression) => {
                if let Some(ty) = self.compile_expression(expression)? {
                    self.emit(
                        Opcode::Drop,
                        Operand::Drop(ty.clone()),
                        StackEffect {
                            pops: vec![stack_value(&ty, false)],
                            pushes: vec![],
                        },
                        Vec::new(),
                        Vec::new(),
                        &statement.info,
                    );
                }
                Ok(())
            }
            StatementKind::Return(expression) => {
                let expected = self.procedure.return_type.clone().ok_or_else(|| {
                    vec![diagnostic(
                        "expression Return is invalid in a Sub",
                        &statement.info,
                    )]
                })?;
                let actual = self.compile_expression(expression)?.ok_or_else(|| {
                    vec![diagnostic(
                        "Function Return expression produced no value",
                        &expression.info,
                    )]
                })?;
                if actual != expected {
                    return Err(vec![diagnostic(
                        "HIR Return value lacks an explicit conversion to the Function result type",
                        &expression.info,
                    )]);
                }
                self.emit(
                    Opcode::ReturnValue,
                    Operand::None,
                    StackEffect {
                        pops: vec![stack_value(&expected, false)],
                        pushes: vec![],
                    },
                    Vec::new(),
                    vec![SideEffect::ControlFlow],
                    &statement.info,
                );
                Ok(())
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                let condition_type = self.compile_expression(condition)?.ok_or_else(|| {
                    vec![diagnostic(
                        "If condition produced no value",
                        &condition.info,
                    )]
                })?;
                if condition_type.kind == VBType::Variant {
                    self.emit_convert(&condition_type, &TypeInfo::boolean(), &condition.info);
                } else if condition_type.kind != VBType::Boolean {
                    return Err(vec![diagnostic(
                        "If condition is not Boolean-convertible in typed HIR",
                        &condition.info,
                    )]);
                }
                let otherwise = self.new_label();
                let end = self.new_label();
                self.emit_branch(Opcode::JumpIfFalse, otherwise, &statement.info);
                for child in then_body {
                    self.compile_statement(child)?;
                }
                self.emit_branch(Opcode::Jump, end, &statement.info);
                self.bind(otherwise)?;
                for child in else_body {
                    self.compile_statement(child)?;
                }
                self.bind(end)
            }
            StatementKind::Unsupported { syntax } => Err(vec![diagnostic(
                &format!("unsupported HIR statement: {syntax}"),
                &statement.info,
            )]),
            StatementKind::Loop {
                loop_kind,
                condition,
                post_test,
                until,
                body,
            } => self.compile_loop(
                *loop_kind,
                condition.as_ref(),
                *post_test,
                *until,
                body,
                &statement.info,
            ),
            StatementKind::For {
                variable,
                start,
                end,
                step,
                body,
            } => self.compile_for(variable, start, end, step.as_ref(), body, &statement.info),
            StatementKind::Continue(target) => {
                let label = self
                    .active_loop_targets
                    .iter()
                    .rev()
                    .find_map(|(active, label)| (active == target).then_some(*label))
                    .ok_or_else(|| {
                        vec![diagnostic(
                            &format!("Continue {target:?} has no compiled active loop"),
                            &statement.info,
                        )]
                    })?;
                self.emit_branch(Opcode::Jump, label, &statement.info);
                Ok(())
            }
            StatementKind::Select { .. } => Err(vec![diagnostic(
                "Select Case lowering is not yet emitted",
                &statement.info,
            )]),
            StatementKind::Try { .. }
            | StatementKind::ShortCircuit { .. }
            | StatementKind::TaskContinuation { .. } => Err(vec![diagnostic(
                "MX control-flow lowering is not yet emitted",
                &statement.info,
            )]),
        }
    }

    fn compile_loop(
        &mut self,
        loop_kind: LoopKind,
        condition: Option<&Expression>,
        post_test: bool,
        until: bool,
        body: &[Statement],
        info: &NodeInfo,
    ) -> Result<(), Vec<CompileDiagnostic>> {
        let start = self.new_label();
        let test = self.new_label();
        let end = self.new_label();
        if !post_test {
            self.bind(start)?;
            if let Some(condition) = condition {
                self.emit_loop_condition(condition, until, end)?;
            }
        }
        if loop_kind == LoopKind::Do {
            self.active_loop_targets
                .push((ContinueTarget::Do, if post_test { test } else { start }));
        }
        for statement in body {
            self.compile_statement(statement)?;
        }
        if loop_kind == LoopKind::Do {
            self.active_loop_targets.pop();
        }
        if post_test {
            self.bind(test)?;
            if let Some(condition) = condition {
                self.emit_loop_condition(condition, until, end)?;
            }
        }
        self.emit_branch(Opcode::Jump, start, info);
        self.bind(end)
    }

    fn emit_loop_condition(
        &mut self,
        condition: &Expression,
        until: bool,
        end: Label,
    ) -> Result<(), Vec<CompileDiagnostic>> {
        let ty = self.compile_expression(condition)?.ok_or_else(|| {
            vec![diagnostic(
                "loop condition produced no value",
                &condition.info,
            )]
        })?;
        if ty.kind == VBType::Variant {
            self.emit_convert(&ty, &TypeInfo::boolean(), &condition.info);
        } else if ty.kind != VBType::Boolean {
            return Err(vec![diagnostic(
                "loop condition is not Boolean-convertible in typed HIR",
                &condition.info,
            )]);
        }
        self.emit_branch(
            if until {
                Opcode::JumpIfTrue
            } else {
                Opcode::JumpIfFalse
            },
            end,
            &condition.info,
        );
        Ok(())
    }

    fn compile_for(
        &mut self,
        variable: &Expression,
        start: &Expression,
        end: &Expression,
        step: Option<&Expression>,
        body: &[Statement],
        info: &NodeInfo,
    ) -> Result<(), Vec<CompileDiagnostic>> {
        let counter = self.resolve_variable(variable).ok_or_else(|| {
            vec![diagnostic(
                "For counter does not resolve to a variable slot",
                &variable.info,
            )]
        })?;
        let counter_type = self
            .variable_type(counter)
            .ok_or_else(|| vec![diagnostic("For counter slot is missing", &variable.info)])?;
        let loop_id = self.next_hidden_loop;
        self.next_hidden_loop = self
            .next_hidden_loop
            .checked_add(1)
            .ok_or_else(|| vec![diagnostic("too many For loops in one procedure", info)])?;
        let limit_slot =
            self.allocate_hidden_local(&format!("$for{loop_id}.limit"), &counter_type, info)?;
        let step_slot =
            self.allocate_hidden_local(&format!("$for{loop_id}.step"), &counter_type, info)?;

        let start_type = self
            .compile_expression(start)?
            .ok_or_else(|| vec![diagnostic("For start produced no value", &start.info)])?;
        let end_type = self
            .compile_expression(end)?
            .ok_or_else(|| vec![diagnostic("For limit produced no value", &end.info)])?;
        let step_type = if let Some(step) = step {
            self.compile_expression(step)?
                .ok_or_else(|| vec![diagnostic("For step produced no value", &step.info)])?
        } else {
            let constant = Constant {
                ty: counter_type.clone(),
                value: one_constant(&counter_type).ok_or_else(|| {
                    vec![diagnostic(
                        "default For step is unsupported for this numeric type",
                        info,
                    )]
                })?,
            };
            let index = self.intern_constant(constant.clone());
            self.emit(
                Opcode::PushConstant,
                Operand::Constant(index),
                StackEffect {
                    pops: vec![],
                    pushes: vec![stack_value(&constant.ty, false)],
                },
                Vec::new(),
                Vec::new(),
                info,
            );
            counter_type.clone()
        };
        if start_type != counter_type || end_type != counter_type || step_type != counter_type {
            return Err(vec![diagnostic(
                "For bounds and step must match the counter type after HIR conversions",
                info,
            )]);
        }

        let operands = Operand::ForLoop {
            counter,
            limit_slot,
            step_slot,
        };
        self.emit(
            Opcode::ForInitialize,
            operands.clone(),
            StackEffect {
                pops: vec![
                    stack_value(&counter_type, false),
                    stack_value(&counter_type, false),
                    stack_value(&counter_type, false),
                ],
                pushes: vec![],
            },
            vec![
                counter,
                VariableRef::Local(limit_slot),
                VariableRef::Local(step_slot),
            ],
            vec![SideEffect::WriteVariable],
            info,
        );
        let test = self.new_label();
        let advance = self.new_label();
        let end_label = self.new_label();
        self.bind(test)?;
        self.emit(
            Opcode::ForTest,
            operands.clone(),
            StackEffect {
                pops: vec![],
                pushes: vec![stack_value(&TypeInfo::boolean(), false)],
            },
            vec![
                counter,
                VariableRef::Local(limit_slot),
                VariableRef::Local(step_slot),
            ],
            vec![SideEffect::ReadVariable],
            info,
        );
        self.emit_branch(Opcode::JumpIfFalse, end_label, info);
        self.active_loop_targets
            .push((ContinueTarget::For, advance));
        for statement in body {
            self.compile_statement(statement)?;
        }
        self.active_loop_targets.pop();
        self.bind(advance)?;
        self.emit(
            Opcode::ForAdvance,
            operands,
            StackEffect::default(),
            vec![counter, VariableRef::Local(step_slot)],
            vec![
                SideEffect::ReadVariable,
                SideEffect::WriteVariable,
                SideEffect::MayRaise,
            ],
            info,
        );
        self.emit_branch(Opcode::Jump, test, info);
        self.bind(end_label)
    }

    fn allocate_hidden_local(
        &mut self,
        name: &str,
        ty: &TypeInfo,
        info: &NodeInfo,
    ) -> Result<u16, Vec<CompileDiagnostic>> {
        let index = self.procedure.parameters.len() + self.procedure.locals.len();
        let index = u16::try_from(index)
            .map_err(|_| vec![diagnostic("procedure frame exceeds u16 slot limit", info)])?;
        self.procedure.locals.push(VariableSlot {
            index,
            name: name.to_string(),
            ty: ty.clone(),
            kind: SlotKind::Local,
            optional: false,
            symbol: SymbolRef {
                scope_id: self.procedure.symbol.scope_id,
                name: name.to_ascii_lowercase(),
            },
        });
        Ok(index)
    }

    fn intern_constant(&mut self, constant: Constant) -> u32 {
        if let Some(index) = self
            .module
            .constants
            .iter()
            .position(|candidate| candidate == &constant)
        {
            index as u32
        } else {
            let index = self.module.constants.len();
            self.module.constants.push(constant);
            index as u32
        }
    }

    fn compile_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<Option<TypeInfo>, Vec<CompileDiagnostic>> {
        match &expression.kind {
            ExpressionKind::Literal(text) => {
                let ty = expression.info.ty.clone().ok_or_else(|| {
                    vec![diagnostic("literal has no static type", &expression.info)]
                })?;
                let value = literal_value(text, &ty).ok_or_else(|| {
                    vec![diagnostic(
                        "literal encoding is not supported",
                        &expression.info,
                    )]
                })?;
                let constant = Constant {
                    ty: ty.clone(),
                    value,
                };
                let index = self.intern_constant(constant);
                self.emit(
                    Opcode::PushConstant,
                    Operand::Constant(index),
                    StackEffect {
                        pops: vec![],
                        pushes: vec![stack_value(&ty, false)],
                    },
                    Vec::new(),
                    Vec::new(),
                    &expression.info,
                );
                Ok(Some(ty))
            }
            ExpressionKind::Name(_) => {
                if let Some(variable) = self.resolve_variable(expression) {
                    let ty = self.variable_type(variable).ok_or_else(|| {
                        vec![diagnostic("variable slot is missing", &expression.info)]
                    })?;
                    let opcode = match variable {
                        VariableRef::Local(_) => Opcode::LoadLocal,
                        VariableRef::Module(_) => Opcode::LoadModule,
                    };
                    self.emit(
                        opcode,
                        Operand::Variable(variable),
                        StackEffect {
                            pops: vec![],
                            pushes: vec![stack_value(&ty, false)],
                        },
                        vec![variable],
                        vec![SideEffect::ReadVariable],
                        &expression.info,
                    );
                    Ok(Some(ty))
                } else {
                    Err(vec![diagnostic(
                        "name is not a compilable variable value",
                        &expression.info,
                    )])
                }
            }
            ExpressionKind::Parenthesized(inner) => self.compile_expression(inner),
            ExpressionKind::Conversion {
                expression: inner,
                target_type,
                ..
            } => {
                let from = self.compile_expression(inner)?.ok_or_else(|| {
                    vec![diagnostic(
                        "conversion source produced no value",
                        &inner.info,
                    )]
                })?;
                let to = value_type(target_type);
                if from != to {
                    self.emit_convert(&from, &to, &expression.info);
                }
                Ok(Some(to))
            }
            ExpressionKind::Unary { operator, operand } => {
                let operator = operator_name(operator).ok_or_else(|| {
                    vec![diagnostic("unsupported unary operator", &expression.info)]
                })?;
                if !matches!(operator.as_str(), "+" | "-") {
                    return Err(vec![diagnostic(
                        "unary operator is not in the verified VB6R subset",
                        &expression.info,
                    )]);
                }
                let operand_type = self.compile_expression(operand)?.ok_or_else(|| {
                    vec![diagnostic("unary operand produced no value", &operand.info)]
                })?;
                let result_type = expression.info.ty.as_ref().map(value_type).ok_or_else(|| {
                    vec![diagnostic(
                        "unary expression has no result type",
                        &expression.info,
                    )]
                })?;
                if !canonical::supports_unary(&operator, &operand_type, &result_type) {
                    return Err(vec![diagnostic(
                        "unary operator/type combination is outside canonical P-Code v1",
                        &expression.info,
                    )]);
                }
                self.emit(
                    Opcode::Unary,
                    Operand::Unary {
                        operator,
                        operand_type: operand_type.clone(),
                        result_type: result_type.clone(),
                    },
                    StackEffect {
                        pops: vec![stack_value(&operand_type, false)],
                        pushes: vec![stack_value(&result_type, false)],
                    },
                    Vec::new(),
                    vec![SideEffect::MayRaise],
                    &expression.info,
                );
                Ok(Some(result_type))
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                let operator = operator_name(operator).ok_or_else(|| {
                    vec![diagnostic("unsupported binary operator", &expression.info)]
                })?;
                if !matches!(
                    operator.to_ascii_lowercase().as_str(),
                    "+" | "-" | "*" | "/" | "=" | "<>" | "<" | "<=" | ">" | ">=" | "&"
                ) {
                    return Err(vec![diagnostic(
                        "binary operator is not in the verified VB6R subset",
                        &expression.info,
                    )]);
                }
                let left_type = self.compile_expression(left)?.ok_or_else(|| {
                    vec![diagnostic("left operand produced no value", &left.info)]
                })?;
                let right_type = self.compile_expression(right)?.ok_or_else(|| {
                    vec![diagnostic("right operand produced no value", &right.info)]
                })?;
                let result_type = expression.info.ty.as_ref().map(value_type).ok_or_else(|| {
                    vec![diagnostic(
                        "binary expression has no result type",
                        &expression.info,
                    )]
                })?;
                if !canonical::supports_binary(&operator, &left_type, &right_type, &result_type) {
                    return Err(vec![diagnostic(
                        "binary operator/type combination is outside canonical P-Code v1",
                        &expression.info,
                    )]);
                }
                self.emit(
                    Opcode::Binary,
                    Operand::Binary {
                        operator,
                        left_type: left_type.clone(),
                        right_type: right_type.clone(),
                        result_type: result_type.clone(),
                    },
                    StackEffect {
                        pops: vec![
                            stack_value(&right_type, false),
                            stack_value(&left_type, false),
                        ],
                        pushes: vec![stack_value(&result_type, false)],
                    },
                    Vec::new(),
                    vec![SideEffect::MayRaise],
                    &expression.info,
                );
                Ok(Some(result_type))
            }
            ExpressionKind::Call { callee, arguments } => {
                self.compile_call(expression, callee, arguments)
            }
            ExpressionKind::Coalesce { .. } | ExpressionKind::ShortCircuit { .. } => {
                Err(vec![diagnostic(
                    "lazy MX expression has no canonical emitted opcode yet",
                    &expression.info,
                )])
            }
            ExpressionKind::Member { .. } => Err(vec![diagnostic(
                "member/property access lowering is not emitted",
                &expression.info,
            )]),
            ExpressionKind::Lambda { .. } | ExpressionKind::Spread(_) => Err(vec![diagnostic(
                "lambda/spread lowering is not emitted",
                &expression.info,
            )]),
        }
    }

    fn compile_call(
        &mut self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[Expression],
    ) -> Result<Option<TypeInfo>, Vec<CompileDiagnostic>> {
        let ExpressionKind::Name(name) = &callee.kind else {
            return Err(vec![diagnostic(
                "only direct named procedure calls are emitted",
                &callee.info,
            )]);
        };
        let key = name.to_ascii_lowercase();
        let id = *self.module.proc_ids.get(&key).ok_or_else(|| {
            vec![diagnostic(
                "call target is not a procedure in this module",
                &callee.info,
            )]
        })?;
        let formal = self
            .module
            .signatures
            .get(&key)
            .ok_or_else(|| vec![diagnostic("call signature is missing", &callee.info)])?;
        if arguments.len() != formal.signature.parameters.len() {
            return Err(vec![diagnostic(
                "optional and ParamArray argument expansion is not emitted",
                &expression.info,
            )]);
        }
        let formals = formal.signature.parameters.clone();
        let return_type = formal.signature.return_type.as_ref().map(value_type);
        for (argument, parameter) in arguments.iter().zip(&formals) {
            match parameter.passing {
                PassingMode::ByVal => {
                    self.compile_expression(argument)?.ok_or_else(|| {
                        vec![diagnostic(
                            "ByVal argument produced no value",
                            &argument.info,
                        )]
                    })?;
                }
                PassingMode::ByRef => {
                    let source = byref_source(argument).ok_or_else(|| {
                        vec![diagnostic(
                            "ByRef argument must be an addressable variable without a value conversion",
                            &argument.info,
                        )]
                    })?;
                    let ExpressionKind::Name(_) = &source.kind else {
                        return Err(vec![diagnostic(
                            "ByRef argument must be a variable name",
                            &source.info,
                        )]);
                    };
                    let variable = self.resolve_variable(source).ok_or_else(|| {
                        vec![diagnostic(
                            "ByRef argument does not resolve to a variable slot",
                            &source.info,
                        )]
                    })?;
                    let actual_type = self.variable_type(variable).ok_or_else(|| {
                        vec![diagnostic("ByRef variable slot is missing", &source.info)]
                    })?;
                    let formal_type = value_type(&parameter.ty);
                    if actual_type != formal_type {
                        return Err(vec![diagnostic(
                            "ByRef arguments require an exact static type in this compiler tranche",
                            &source.info,
                        )]);
                    }
                    let opcode = match variable {
                        VariableRef::Local(_) => Opcode::AddressLocal,
                        VariableRef::Module(_) => Opcode::AddressModule,
                    };
                    self.emit(
                        opcode,
                        Operand::Variable(variable),
                        StackEffect {
                            pops: vec![],
                            pushes: vec![stack_value(&actual_type, true)],
                        },
                        vec![variable],
                        vec![SideEffect::ReadVariable],
                        &source.info,
                    );
                }
                PassingMode::ParamArray => {
                    return Err(vec![diagnostic(
                        "ParamArray packing is not emitted",
                        &argument.info,
                    )]);
                }
            }
        }
        let pops = formals
            .iter()
            .rev()
            .map(|parameter| {
                stack_value(
                    &value_type(&parameter.ty),
                    parameter.passing == PassingMode::ByRef,
                )
            })
            .collect();
        let pushes = return_type
            .iter()
            .map(|ty| stack_value(ty, false))
            .collect();
        self.emit(
            Opcode::Call,
            Operand::Call(id),
            StackEffect { pops, pushes },
            Vec::new(),
            vec![SideEffect::Call, SideEffect::MayRaise],
            &expression.info,
        );
        Ok(return_type)
    }

    fn emit_convert(&mut self, from: &TypeInfo, to: &TypeInfo, info: &NodeInfo) {
        self.emit(
            Opcode::Convert,
            Operand::Convert {
                from: from.clone(),
                to: to.clone(),
            },
            StackEffect {
                pops: vec![stack_value(from, false)],
                pushes: vec![stack_value(to, false)],
            },
            Vec::new(),
            vec![SideEffect::MayRaise],
            info,
        );
    }

    fn emit_branch(&mut self, opcode: Opcode, label: Label, info: &NodeInfo) {
        let index = self.procedure.instructions.len();
        let (effect, effects) = if opcode == Opcode::JumpIfFalse {
            (
                StackEffect {
                    pops: vec![stack_value(&TypeInfo::boolean(), false)],
                    pushes: vec![],
                },
                vec![SideEffect::ControlFlow],
            )
        } else {
            (StackEffect::default(), vec![SideEffect::ControlFlow])
        };
        self.emit(
            opcode,
            Operand::Branch(u32::MAX),
            effect,
            Vec::new(),
            effects,
            info,
        );
        self.pending_branches.push((index, label));
    }

    fn emit(
        &mut self,
        opcode: Opcode,
        operand: Operand,
        stack: StackEffect,
        variables: Vec<VariableRef>,
        effects: Vec<SideEffect>,
        info: &NodeInfo,
    ) {
        let instruction = self.procedure.instructions.len() as u32;
        let source_map_index = self.procedure.source_map.len() as u32;
        self.procedure
            .source_map
            .push(source_entry(instruction, info));
        self.procedure.instructions.push(Instruction {
            opcode,
            operand,
            stack,
            variables,
            effects,
            source_map_index,
        });
    }

    fn new_label(&mut self) -> Label {
        let label = Label(self.next_label);
        self.next_label += 1;
        label
    }

    fn bind(&mut self, label: Label) -> Result<(), Vec<CompileDiagnostic>> {
        if self
            .labels
            .insert(label, self.procedure.instructions.len() as u32)
            .is_some()
        {
            return Err(vec![CompileDiagnostic {
                message: "duplicate symbolic label".to_string(),
                location: None,
            }]);
        }
        Ok(())
    }

    fn resolve_branches(&mut self) -> Result<(), Vec<CompileDiagnostic>> {
        for (instruction, label) in &self.pending_branches {
            let target = self.labels.get(label).copied().ok_or_else(|| {
                vec![CompileDiagnostic {
                    message: format!("unbound symbolic branch label {}", label.0),
                    location: None,
                }]
            })?;
            let Some(code) = self.procedure.instructions.get_mut(*instruction) else {
                return Err(vec![CompileDiagnostic {
                    message: "branch instruction index is invalid".to_string(),
                    location: None,
                }]);
            };
            code.operand = Operand::Branch(target);
        }
        Ok(())
    }

    fn resolve_variable(&self, expression: &Expression) -> Option<VariableRef> {
        let symbol = expression.info.symbol.as_ref().map(symbol_ref)?;
        self.local_ids.get(&symbol).copied().or_else(|| {
            self.module
                .global_ids
                .get(&symbol)
                .copied()
                .map(VariableRef::Module)
        })
    }

    fn variable_type(&self, variable: VariableRef) -> Option<TypeInfo> {
        match variable {
            VariableRef::Local(index) => self
                .procedure
                .parameters
                .iter()
                .chain(&self.procedure.locals)
                .find(|slot| slot.index == index)
                .map(|slot| slot.ty.clone()),
            VariableRef::Module(index) => self
                .module
                .globals
                .get(index as usize)
                .map(|slot| slot.ty.clone()),
        }
    }
}

fn module_variable(
    declaration: &Declaration,
    index: usize,
) -> Result<ModuleVariable, CompileDiagnostic> {
    let DeclarationKind::Variable { constant: false } = &declaration.kind else {
        return Err(diagnostic(
            "module constants lack initializer values in HIR",
            &declaration.info,
        ));
    };
    let (Some(symbol), Some(ty)) = (
        declaration.info.symbol.as_ref().map(symbol_ref),
        declaration.info.ty.as_ref().map(value_type),
    ) else {
        return Err(diagnostic(
            "module variable lacks a bound symbol or static type",
            &declaration.info,
        ));
    };
    let index = u16::try_from(index).map_err(|_| {
        diagnostic(
            "module variable table exceeds u16 slot limit",
            &declaration.info,
        )
    })?;
    Ok(ModuleVariable {
        index,
        name: declaration.name.clone(),
        ty,
        symbol,
    })
}

fn variable_slot(
    parameter: &Parameter,
    index: usize,
    kind: SlotKind,
) -> Result<VariableSlot, Vec<CompileDiagnostic>> {
    let symbol = parameter
        .info
        .symbol
        .as_ref()
        .map(symbol_ref)
        .ok_or_else(|| {
            vec![diagnostic(
                "parameter lacks a bound semantic symbol",
                &parameter.info,
            )]
        })?;
    let index = u16::try_from(index).map_err(|_| {
        vec![diagnostic(
            "procedure frame exceeds u16 slot limit",
            &parameter.info,
        )]
    })?;
    Ok(VariableSlot {
        index,
        name: parameter.name.clone(),
        ty: value_type(&parameter.ty),
        kind,
        optional: parameter.optional,
        symbol,
    })
}

fn variable_slot_from_declaration(
    declaration: &Declaration,
    index: usize,
) -> Result<VariableSlot, CompileDiagnostic> {
    let (Some(symbol), Some(ty)) = (
        declaration.info.symbol.as_ref().map(symbol_ref),
        declaration.info.ty.as_ref().map(value_type),
    ) else {
        return Err(diagnostic(
            "local variable lacks a bound semantic symbol or static type",
            &declaration.info,
        ));
    };
    let index = u16::try_from(index)
        .map_err(|_| diagnostic("procedure frame exceeds u16 slot limit", &declaration.info))?;
    Ok(VariableSlot {
        index,
        name: declaration.name.clone(),
        ty,
        kind: SlotKind::Local,
        optional: false,
        symbol,
    })
}

fn collect_local_declarations(
    statements: &[Statement],
    visit: &mut impl FnMut(&Declaration) -> Result<(), CompileDiagnostic>,
) -> Result<(), Vec<CompileDiagnostic>> {
    for statement in statements {
        match &statement.kind {
            StatementKind::Declaration(declaration) => visit(declaration).map_err(|e| vec![e])?,
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_declarations(then_body, visit)?;
                collect_local_declarations(else_body, visit)?;
            }
            StatementKind::For { body, .. } | StatementKind::Loop { body, .. } => {
                collect_local_declarations(body, visit)?
            }
            StatementKind::Select { cases, .. } => {
                for case in cases {
                    collect_local_declarations(&case.body, visit)?;
                }
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => {
                collect_local_declarations(body, visit)?;
                for catch in catches {
                    collect_local_declarations(catch, visit)?;
                }
                collect_local_declarations(finally_body, visit)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn parameter_kind(passing: PassingMode) -> SlotKind {
    match passing {
        PassingMode::ByVal => SlotKind::ByValParameter,
        PassingMode::ByRef => SlotKind::ByRefParameter,
        PassingMode::ParamArray => SlotKind::ParamArray,
    }
}

fn symbol_ref(symbol: &vb6semantic::SymbolId) -> SymbolRef {
    SymbolRef {
        scope_id: symbol.scope_id,
        name: symbol.name.clone(),
    }
}

fn stack_value(ty: &TypeInfo, by_ref: bool) -> StackValue {
    StackValue {
        ty: ty.clone(),
        by_ref,
    }
}

fn value_type(ty: &TypeInfo) -> TypeInfo {
    let mut value = ty.clone();
    value.is_reference = false;
    value
}

fn byref_source(expression: &Expression) -> Option<&Expression> {
    let mut source = expression;
    loop {
        match &source.kind {
            ExpressionKind::Conversion {
                expression: inner,
                target_type,
                ..
            } if value_type(target_type) == value_type(inner.info.ty.as_ref()?) => {
                source = inner;
            }
            _ => return Some(source),
        }
    }
}

fn source_entry(instruction: u32, info: &NodeInfo) -> SourceMapEntry {
    SourceMapEntry {
        instruction,
        span: (info.span.start, info.span.end),
        file: info.location.file.clone(),
        line: info.location.line,
        column: info.location.column,
        symbol: info.symbol.as_ref().map(symbol_ref),
    }
}

fn operator_name(operator: &Operator) -> Option<String> {
    match operator {
        Operator::Source(source) => Some(source.clone()),
    }
}

fn literal_value(text: &str, ty: &TypeInfo) -> Option<ConstantValue> {
    match &ty.kind {
        VBType::Byte | VBType::Integer | VBType::Long => {
            let clean = text.trim().trim_end_matches(['&', '%', '^']);
            let integer = clean.parse::<i64>().ok()?;
            match ty.kind {
                VBType::Byte if !(0..=u8::MAX as i64).contains(&integer) => None,
                VBType::Integer if !(i16::MIN as i64..=i16::MAX as i64).contains(&integer) => None,
                VBType::Long if !(i32::MIN as i64..=i32::MAX as i64).contains(&integer) => None,
                _ => Some(ConstantValue::Integer(integer)),
            }
        }
        VBType::Single => {
            let value = text.trim().trim_end_matches('!').parse::<f32>().ok()?;
            Some(ConstantValue::FloatBits(u64::from(value.to_bits())))
        }
        VBType::Double => Some(ConstantValue::FloatBits(
            text.trim()
                .trim_end_matches('#')
                .parse::<f64>()
                .ok()?
                .to_bits(),
        )),
        VBType::Currency | VBType::Decimal => Some(ConstantValue::DecimalText(
            text.trim().trim_end_matches(['@', 'D', 'd']).to_string(),
        )),
        VBType::String => {
            let value = text
                .strip_prefix('"')?
                .strip_suffix('"')?
                .replace("\"\"", "\"");
            Some(ConstantValue::String(value))
        }
        VBType::Boolean => Some(ConstantValue::Boolean(
            text.trim().eq_ignore_ascii_case("True"),
        )),
        VBType::Nothing => Some(ConstantValue::Sentinel(Sentinel::Nothing)),
        VBType::Empty => Some(ConstantValue::Sentinel(Sentinel::Empty)),
        VBType::Null => Some(ConstantValue::Sentinel(Sentinel::Null)),
        _ => None,
    }
}

fn one_constant(ty: &TypeInfo) -> Option<ConstantValue> {
    match ty.kind {
        VBType::Byte | VBType::Integer | VBType::Long => Some(ConstantValue::Integer(1)),
        VBType::Single => Some(ConstantValue::FloatBits(u64::from(1.0_f32.to_bits()))),
        VBType::Double => Some(ConstantValue::FloatBits(1.0_f64.to_bits())),
        VBType::Currency | VBType::Decimal => Some(ConstantValue::DecimalText("1".to_string())),
        _ => None,
    }
}

fn diagnostic(message: &str, info: &NodeInfo) -> CompileDiagnostic {
    CompileDiagnostic {
        message: message.to_string(),
        location: Some(info.location.clone()),
    }
}
