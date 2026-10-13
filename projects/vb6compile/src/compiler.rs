use vb6core::types::TypeInfo;
use vb6parse::files::ModuleFile;
use vb6parse::io::SourceFile;
use vb6pcode::program::{
    self, Constant, FrameSlot, Instruction, Operand, PROGRAM_FORMAT_VERSION, ParameterPassing,
    ParameterSlot, ProcedureCode, Program, SideEffect, SourceMapEntry, StackEffect, StackValue,
    SymbolRef, primary_opcode,
};
use vb6semantic::{
    ExpressionKind, HirAnalysis, NodeInfo, Operator, PassingMode, Procedure, StatementKind,
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

/// Compile a typed HIR module to the currently supported Microsoft P-Code subset.
pub fn compile_hir(analysis: &HirAnalysis) -> Result<Program, Vec<CompileDiagnostic>> {
    let Some(typed) = analysis.program.as_ref() else {
        return Err(analysis
            .errors
            .iter()
            .map(|error| CompileDiagnostic {
                message: error.to_string(),
                location: None,
            })
            .collect());
    };
    if !typed.compilable || !analysis.errors.is_empty() {
        return Err(vec![CompileDiagnostic {
            message: "semantic analysis did not produce compilable HIR".to_string(),
            location: Some(typed.info.location.clone()),
        }]);
    }
    PCodeBuilder::new().build(typed)
}

/// Parse, analyze, and compile one VB6 source module.
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

/// Builder for the modern module container and the verified Microsoft Add(I4) subset.
#[derive(Debug, Default)]
pub struct PCodeBuilder;

impl PCodeBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self
    }

    /// Lower one `Function name(a As Long, b As Long) As Long` whose body assigns
    /// `name = a + b`; other HIR forms return a diagnostic.
    pub fn build(&self, typed: &TypedProgram) -> Result<Program, Vec<CompileDiagnostic>> {
        if !typed.declarations.is_empty() || typed.procedures.len() != 1 {
            return Err(vec![diagnostic(
                "only one Function with the supported Add(Long, Long) body is implemented",
                &typed.info,
            )]);
        }
        let procedure = &typed.procedures[0];
        let code = compile_add_function(procedure)?;
        let mut program = Program {
            format_version: PROGRAM_FORMAT_VERSION,
            module_name: typed.name.clone(),
            constants: Vec::<Constant>::new(),
            procedures: vec![code],
        };
        program::validate(&mut program).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| CompileDiagnostic {
                    message: format!("generated Microsoft P-Code failed validation: {error}"),
                    location: Some(procedure.info.location.clone()),
                })
                .collect::<Vec<_>>()
        })?;
        Ok(program)
    }
}

fn compile_add_function(procedure: &Procedure) -> Result<ProcedureCode, Vec<CompileDiagnostic>> {
    if procedure.signature.return_type.as_ref().map(value_type) != Some(TypeInfo::long()) {
        return Err(vec![diagnostic(
            "the supported Microsoft Add sequence requires a Long Function result",
            &procedure.info,
        )]);
    }
    if procedure.signature.parameters.len() != 2
        || procedure
            .signature
            .parameters
            .iter()
            .any(|parameter| value_type(&parameter.ty) != TypeInfo::long())
    {
        return Err(vec![diagnostic(
            "the supported Microsoft Add sequence requires exactly two Long parameters",
            &procedure.info,
        )]);
    }
    if procedure.body.len() != 1 {
        return Err(vec![diagnostic(
            "the supported Microsoft Add sequence requires one result assignment",
            &procedure.info,
        )]);
    }
    let StatementKind::Assignment { target, value } = &procedure.body[0].kind else {
        return Err(vec![diagnostic(
            "the supported Microsoft Add body must assign the Function result",
            &procedure.body[0].info,
        )]);
    };
    if !matches!(&target.kind, ExpressionKind::Name(name) if name.eq_ignore_ascii_case(&procedure.name))
    {
        return Err(vec![diagnostic(
            "the assignment target must be the Long Function result",
            &target.info,
        )]);
    }
    let ExpressionKind::Binary {
        operator: Operator::Source(operator),
        left,
        right,
    } = &value.kind
    else {
        return Err(vec![diagnostic(
            "the result expression must be a binary addition",
            &value.info,
        )]);
    };
    if operator != "+" || value.info.ty.as_ref().map(value_type) != Some(TypeInfo::long()) {
        return Err(vec![diagnostic(
            "the result expression must be Long addition",
            &value.info,
        )]);
    }
    for (argument, parameter) in [left.as_ref(), right.as_ref()]
        .into_iter()
        .zip(&procedure.signature.parameters)
    {
        if !matches!(&argument.kind, ExpressionKind::Name(name) if name.eq_ignore_ascii_case(&parameter.name))
            || argument.info.symbol.as_ref().map(symbol_ref)
                != parameter.info.symbol.as_ref().map(symbol_ref)
            || argument.info.ty.as_ref().map(value_type) != Some(TypeInfo::long())
        {
            return Err(vec![diagnostic(
                "the addition operands must be the declared Long parameters in order",
                &argument.info,
            )]);
        }
    }

    let symbol = procedure
        .info
        .symbol
        .as_ref()
        .map(symbol_ref)
        .ok_or_else(|| {
            vec![diagnostic(
                "Function has no bound semantic symbol",
                &procedure.info,
            )]
        })?;
    let parameters = procedure
        .signature
        .parameters
        .iter()
        .enumerate()
        .map(|(slot, parameter)| {
            let slot = u16::try_from(slot).map_err(|_| {
                vec![diagnostic(
                    "procedure frame exceeds u16 slots",
                    &parameter.info,
                )]
            })?;
            let symbol = parameter
                .info
                .symbol
                .as_ref()
                .map(symbol_ref)
                .ok_or_else(|| {
                    vec![diagnostic(
                        "parameter has no bound semantic symbol",
                        &parameter.info,
                    )]
                })?;
            Ok(ParameterSlot {
                slot,
                name: parameter.name.clone(),
                ty: value_type(&parameter.ty),
                passing: match parameter.passing {
                    PassingMode::ByVal => ParameterPassing::ByVal,
                    PassingMode::ByRef => ParameterPassing::ByRef,
                    PassingMode::ParamArray => {
                        return Err(vec![diagnostic(
                            "ParamArray is outside the supported Add signature",
                            &parameter.info,
                        )]);
                    }
                },
                symbol,
            })
        })
        .collect::<Result<Vec<_>, Vec<CompileDiagnostic>>>()?;

    let mut instructions = Vec::with_capacity(5);
    let mut source_map = Vec::with_capacity(5);
    emit(
        &mut instructions,
        &mut source_map,
        0x6c,
        Operand::FrameSlot(FrameSlot::Parameter(0)),
        StackEffect {
            pops: vec![],
            pushes: vec![stack_value(&TypeInfo::long())],
        },
        vec![SideEffect::FrameAccess],
        &left.info,
    );
    emit(
        &mut instructions,
        &mut source_map,
        0x6c,
        Operand::FrameSlot(FrameSlot::Parameter(1)),
        StackEffect {
            pops: vec![],
            pushes: vec![stack_value(&TypeInfo::long())],
        },
        vec![SideEffect::FrameAccess],
        &right.info,
    );
    emit(
        &mut instructions,
        &mut source_map,
        0xaa,
        Operand::None,
        StackEffect {
            pops: vec![
                stack_value(&TypeInfo::long()),
                stack_value(&TypeInfo::long()),
            ],
            pushes: vec![stack_value(&TypeInfo::long())],
        },
        vec![SideEffect::MayRaise],
        &value.info,
    );
    emit(
        &mut instructions,
        &mut source_map,
        0x71,
        Operand::FrameSlot(FrameSlot::FunctionResult),
        StackEffect {
            pops: vec![stack_value(&TypeInfo::long())],
            pushes: vec![],
        },
        vec![SideEffect::FrameAccess],
        &procedure.body[0].info,
    );
    emit(
        &mut instructions,
        &mut source_map,
        0x14,
        Operand::None,
        StackEffect::default(),
        vec![SideEffect::ControlFlow],
        &procedure.info,
    );

    Ok(ProcedureCode {
        id: 0,
        name: procedure.name.clone(),
        symbol,
        parameters,
        return_type: TypeInfo::long(),
        instructions,
        source_map,
        max_stack: 0,
    })
}

fn emit(
    instructions: &mut Vec<Instruction>,
    source_map: &mut Vec<SourceMapEntry>,
    opcode_byte: u8,
    operand: Operand,
    stack: StackEffect,
    effects: Vec<SideEffect>,
    info: &NodeInfo,
) {
    let instruction = instructions.len() as u32;
    source_map.push(SourceMapEntry {
        instruction,
        span: (info.span.start, info.span.end),
        file: info.location.file.clone(),
        line: info.location.line,
        column: info.location.column,
        symbol: info.symbol.as_ref().map(symbol_ref),
    });
    instructions.push(Instruction {
        opcode: primary_opcode(opcode_byte),
        operand,
        stack,
        source_map_index: instruction,
        effects,
    });
}

fn stack_value(ty: &TypeInfo) -> StackValue {
    StackValue {
        ty: ty.clone(),
        by_ref: false,
    }
}

fn value_type(ty: &TypeInfo) -> TypeInfo {
    let mut value = ty.clone();
    value.is_reference = false;
    value
}

fn symbol_ref(symbol: &vb6semantic::SymbolId) -> SymbolRef {
    SymbolRef {
        scope_id: symbol.scope_id,
        name: symbol.name.clone(),
    }
}

fn diagnostic(message: &str, info: &NodeInfo) -> CompileDiagnostic {
    CompileDiagnostic {
        message: message.to_string(),
        location: Some(info.location.clone()),
    }
}
