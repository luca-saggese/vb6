//! Versioned executable VB6R instruction model, separate from Microsoft opcodes.

use std::fmt;
use vb6core::types::{TypeInfo, VBType};

/// Version of the canonical VB6R instruction and module format.
pub const VB6R_FORMAT_VERSION: u16 = 1;

/// Whether a canonical unary operator/type tuple is defined in format version 1.
pub fn supports_unary(operator: &str, operand: &TypeInfo, result: &TypeInfo) -> bool {
    matches!(operator, "+" | "-") && is_numeric(&operand.kind) && is_numeric(&result.kind)
}

/// Whether a canonical binary operator/type tuple is defined in format version 1.
pub fn supports_binary(
    operator: &str,
    left: &TypeInfo,
    right: &TypeInfo,
    result: &TypeInfo,
) -> bool {
    let comparison = matches!(operator, "=" | "<>" | "<" | "<=" | ">" | ">=");
    if comparison {
        return result.kind == VBType::Boolean
            && ((is_numeric(&left.kind) && is_numeric(&right.kind))
                || (left.kind == right.kind
                    && matches!(
                        left.kind,
                        VBType::String | VBType::Boolean | VBType::Date | VBType::Variant
                    )));
    }
    match operator {
        "+" if left.kind == VBType::String && right.kind == VBType::String => {
            result.kind == VBType::String
        }
        "+" | "-" | "*" | "/" => {
            is_numeric(&left.kind) && is_numeric(&right.kind) && is_numeric(&result.kind)
        }
        "&" => {
            left.kind == VBType::String
                && right.kind == VBType::String
                && result.kind == VBType::String
        }
        _ => false,
    }
}

fn is_numeric(ty: &VBType) -> bool {
    matches!(
        ty,
        VBType::Byte
            | VBType::Integer
            | VBType::Long
            | VBType::Single
            | VBType::Double
            | VBType::Currency
            | VBType::Decimal
    )
}

/// Microsoft opcode identifiers and canonical VB6R opcodes occupy distinct namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcodeNamespace {
    /// Original Microsoft dispatch-table opcode.
    Microsoft,
    /// Canonical VB6R instruction.
    Vb6r,
}

/// Canonical instruction operations supported by format version 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Opcode {
    /// Push a module constant.
    PushConstant = 0x8000,
    /// Load a frame variable.
    LoadLocal = 0x8001,
    /// Store a frame variable.
    StoreLocal = 0x8002,
    /// Push an addressable frame variable for a `ByRef` argument.
    AddressLocal = 0x8003,
    /// Load a module variable.
    LoadModule = 0x8004,
    /// Store a module variable.
    StoreModule = 0x8005,
    /// Evaluate a typed unary operation.
    Unary = 0x8006,
    /// Evaluate a typed binary operation.
    Binary = 0x8007,
    /// Apply a checked VB6 conversion.
    Convert = 0x8008,
    /// Unconditional jump to an instruction index in this procedure.
    Jump = 0x8009,
    /// Pop a Boolean/Variant condition and jump when false.
    JumpIfFalse = 0x800a,
    /// Invoke a procedure by stable module-local procedure id.
    Call = 0x800b,
    /// Discard a value.
    Drop = 0x800c,
    /// Return a value from a Function.
    ReturnValue = 0x800d,
    /// Return from a Sub.
    ReturnVoid = 0x800e,
    /// Push the address of a module variable for a `ByRef` argument.
    AddressModule = 0x800f,
    /// Save a numeric For counter's initial, limit, and step values.
    ForInitialize = 0x8010,
    /// Test the inclusive numeric For limit using the sign of the step.
    ForTest = 0x8011,
    /// Advance a numeric For counter by its stored step.
    ForAdvance = 0x8012,
    /// Pop a Boolean condition and jump when true.
    JumpIfTrue = 0x8013,
}

impl Opcode {
    /// The stable numeric identifier within the VB6R opcode namespace.
    pub const fn id(self) -> u16 {
        self as u16
    }

    /// Identifies these opcodes as VB6R-defined rather than Microsoft P-Code.
    pub const fn namespace(self) -> OpcodeNamespace {
        OpcodeNamespace::Vb6r
    }
}

/// Source symbol identity retained for debugging and call metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolRef {
    /// Semantic scope identity.
    pub scope_id: usize,
    /// Case-folded VB6 symbol name.
    pub name: String,
}

/// Parameter passing and variable category in a procedure frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// By-value argument.
    ByValParameter,
    /// By-reference argument.
    ByRefParameter,
    /// Variable-length parameter array.
    ParamArray,
    /// Local variable.
    Local,
}

/// Typed variable slot exposed to a VM and debugger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSlot {
    /// Zero-based slot in its owning table.
    pub index: u16,
    /// Source spelling.
    pub name: String,
    /// Resolved static VB6 type.
    pub ty: TypeInfo,
    /// Parameter mode or local-variable category.
    pub kind: SlotKind,
    /// Whether the argument is optional.
    pub optional: bool,
    /// Bound semantic symbol.
    pub symbol: SymbolRef,
}

/// Source literal stored in the module-wide constant pool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstantValue {
    /// Exact integer value.
    Integer(i64),
    /// IEEE-754 bits, preserving the source type's precision.
    FloatBits(u64),
    /// Decimal/Currency source representation pending VM numeric conversion.
    DecimalText(String),
    /// Decoded UTF-8 text from a VB6 string literal.
    String(String),
    /// Boolean literal.
    Boolean(bool),
    /// Distinct VB6 value state.
    Sentinel(Sentinel),
}

/// Non-numeric sentinel constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sentinel {
    /// Object reference state.
    Nothing,
    /// Uninitialized Variant.
    Empty,
    /// Null Variant.
    Null,
}

/// A typed entry in the module-shared constant pool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Constant {
    /// Literal's static VB6 type.
    pub ty: TypeInfo,
    /// Lossless or explicitly typed literal payload.
    pub value: ConstantValue,
}

/// Module-scope variable storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleVariable {
    /// Zero-based module-variable table index.
    pub index: u16,
    /// Source spelling.
    pub name: String,
    /// Resolved type.
    pub ty: TypeInfo,
    /// Semantic symbol.
    pub symbol: SymbolRef,
}

/// Operand representation for version-1 canonical instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// No operand.
    None,
    /// Index in the module-shared constant pool.
    Constant(u32),
    /// Local or module variable reference.
    Variable(VariableRef),
    /// Typed unary operation.
    Unary {
        /// Source operator spelling.
        operator: String,
        /// Operand type.
        operand_type: TypeInfo,
        /// Result type.
        result_type: TypeInfo,
    },
    /// Typed binary operation.
    Binary {
        /// Source operator spelling.
        operator: String,
        /// Left operand type.
        left_type: TypeInfo,
        /// Right operand type.
        right_type: TypeInfo,
        /// Result type.
        result_type: TypeInfo,
    },
    /// Checked conversion contract.
    Convert {
        /// Source type.
        from: TypeInfo,
        /// Destination type.
        to: TypeInfo,
    },
    /// Resolved branch target (instruction index within this procedure).
    Branch(u32),
    /// Module-local callee and argument contract.
    Call(ProcedureId),
    /// Exact type of a discarded stack value.
    Drop(TypeInfo),
    /// Hidden bound and step slots for a counted loop.
    ForLoop {
        /// Counter variable.
        counter: VariableRef,
        /// Local slot holding the evaluated limit.
        limit_slot: u16,
        /// Local slot holding the evaluated step.
        step_slot: u16,
    },
}

/// Variable table selected by an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariableRef {
    /// Procedure-local frame slot.
    Local(u16),
    /// Module-scope variable slot.
    Module(u16),
}

/// Stable zero-based procedure id within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcedureId(pub u32);

/// Abstract value/reference expected on the VM operand stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackValue {
    /// Static type.
    pub ty: TypeInfo,
    /// True when this is an address passed to a `ByRef` formal.
    pub by_ref: bool,
}

/// Declared stack transformation for one instruction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StackEffect {
    /// Values consumed, in stack-pop order.
    pub pops: Vec<StackValue>,
    /// Values produced, in push order.
    pub pushes: Vec<StackValue>,
}

/// Explicit side effects relevant to debugger stepping and validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideEffect {
    /// Reads a variable slot.
    ReadVariable,
    /// Writes a variable slot.
    WriteVariable,
    /// Calls another procedure.
    Call,
    /// May raise a VB6 runtime error (conversion/arithmetic).
    MayRaise,
    /// Alters instruction pointer.
    ControlFlow,
}

/// Per-instruction contract and source mapping index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// Versioned VB6R opcode.
    pub opcode: Opcode,
    /// Typed opcode operands.
    pub operand: Operand,
    /// Declared stack effect, checked by the verifier.
    pub stack: StackEffect,
    /// Variable slots read or written by this instruction.
    pub variables: Vec<VariableRef>,
    /// Observable effects.
    pub effects: Vec<SideEffect>,
    /// Index in the owning procedure's source map.
    pub source_map_index: u32,
}

/// Source location and semantic scope for a P-Code instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMapEntry {
    /// Instruction index within its procedure.
    pub instruction: u32,
    /// Byte span in the original source file.
    pub span: (u32, u32),
    /// One-based file/line/column information.
    pub file: String,
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
    /// Semantic scope/symbol when available.
    pub symbol: Option<SymbolRef>,
}

/// Procedure signature and executable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcedureCode {
    /// Module-local procedure id.
    pub id: ProcedureId,
    /// Source name.
    pub name: String,
    /// Procedure symbol.
    pub symbol: SymbolRef,
    /// Parameters in declaration order.
    pub parameters: Vec<VariableSlot>,
    /// Locals in frame-slot order.
    pub locals: Vec<VariableSlot>,
    /// `None` for Sub, otherwise the Function result type.
    pub return_type: Option<TypeInfo>,
    /// Executable canonical instructions.
    pub instructions: Vec<Instruction>,
    /// Instruction-to-source map.
    pub source_map: Vec<SourceMapEntry>,
    /// Maximum validated operand-stack depth.
    pub max_stack: u32,
}

/// Executable module using canonical VB6R bytecode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Canonical format version.
    pub format_version: u16,
    /// Module name.
    pub module_name: String,
    /// Module-shared typed literal pool.
    pub constants: Vec<Constant>,
    /// Module-level variables.
    pub variables: Vec<ModuleVariable>,
    /// Procedures in module source order.
    pub procedures: Vec<ProcedureCode>,
}

/// Validation failures in generated or persisted canonical P-Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Unsupported canonical format version.
    UnsupportedVersion(u16),
    /// Invalid or missing source map entry.
    InvalidSourceMap { procedure: String, instruction: u32 },
    /// Operand and opcode do not agree.
    InvalidOperand { procedure: String, instruction: u32 },
    /// Constant reference is out of range or has a mismatched type.
    InvalidConstant { procedure: String, instruction: u32 },
    /// Variable reference is out of range.
    InvalidVariable { procedure: String, instruction: u32 },
    /// Branch target is outside procedure code.
    InvalidBranch {
        procedure: String,
        instruction: u32,
        target: u32,
    },
    /// Stack contract does not match the opcode's defined behavior.
    InvalidStackContract { procedure: String, instruction: u32 },
    /// Operand stack underflow.
    StackUnderflow { procedure: String, instruction: u32 },
    /// Actual stack type/reference mode does not match expected operands.
    StackTypeMismatch { procedure: String, instruction: u32 },
    /// Control-flow paths merge with different stack states.
    InconsistentStackMerge { procedure: String, instruction: u32 },
    /// A reachable path exits code without a return.
    MissingReturn { procedure: String },
    /// Return leaves unconsumed values on the operand stack.
    NonEmptyStackAtReturn { procedure: String, instruction: u32 },
    /// Return opcode does not match procedure signature.
    InvalidReturn { procedure: String, instruction: u32 },
    /// Invalid or duplicate procedure id/name.
    InvalidProcedureTable,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ValidationError {}

/// Recompute and validate canonical instructions, tables, branches, and stack flow.
pub fn validate(program: &mut Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    if program.format_version != VB6R_FORMAT_VERSION {
        errors.push(ValidationError::UnsupportedVersion(program.format_version));
        return Err(errors);
    }
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for (index, variable) in program.variables.iter().enumerate() {
        if variable.index as usize != index {
            errors.push(ValidationError::InvalidVariable {
                procedure: program.module_name.clone(),
                instruction: index as u32,
            });
        }
    }
    for (index, procedure) in program.procedures.iter().enumerate() {
        if procedure.id.0 as usize != index
            || !ids.insert(procedure.id)
            || !names.insert(procedure.name.to_ascii_lowercase())
        {
            errors.push(ValidationError::InvalidProcedureTable);
        }
        for (slot_index, parameter) in procedure.parameters.iter().enumerate() {
            if parameter.index as usize != slot_index {
                errors.push(ValidationError::InvalidVariable {
                    procedure: procedure.name.clone(),
                    instruction: slot_index as u32,
                });
            }
        }
        for (offset, local) in procedure.locals.iter().enumerate() {
            let expected = procedure.parameters.len() + offset;
            if local.index as usize != expected {
                errors.push(ValidationError::InvalidVariable {
                    procedure: procedure.name.clone(),
                    instruction: expected as u32,
                });
            }
        }
    }
    let signatures: std::collections::HashMap<_, _> = program
        .procedures
        .iter()
        .map(|procedure| {
            (
                procedure.id,
                ProcedureSignatureView {
                    parameters: procedure.parameters.clone(),
                    return_type: procedure.return_type.clone(),
                },
            )
        })
        .collect();
    for procedure in &mut program.procedures {
        verify_procedure(
            &program.constants,
            &program.variables,
            procedure,
            &signatures,
            &mut errors,
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Clone)]
struct ProcedureSignatureView {
    parameters: Vec<VariableSlot>,
    return_type: Option<TypeInfo>,
}

fn verify_procedure(
    constants: &[Constant],
    variables: &[ModuleVariable],
    procedure: &mut ProcedureCode,
    procedures: &std::collections::HashMap<ProcedureId, ProcedureSignatureView>,
    errors: &mut Vec<ValidationError>,
) {
    let name = &procedure.name;
    let count = procedure.instructions.len();
    if procedure.source_map.len() != count {
        errors.push(ValidationError::InvalidSourceMap {
            procedure: name.clone(),
            instruction: 0,
        });
    }
    for (index, instruction) in procedure.instructions.iter().enumerate() {
        let pc = index as u32;
        if instruction.source_map_index as usize >= procedure.source_map.len()
            || procedure
                .source_map
                .get(instruction.source_map_index as usize)
                .is_none_or(|entry| entry.instruction != pc)
        {
            errors.push(ValidationError::InvalidSourceMap {
                procedure: name.clone(),
                instruction: pc,
            });
        }
        let contract = expected_effect(constants, variables, procedure, instruction, procedures);
        let Some((effect, variables, effects)) = contract else {
            if matches!(instruction.opcode, Opcode::ReturnValue | Opcode::ReturnVoid) {
                errors.push(ValidationError::InvalidReturn {
                    procedure: name.clone(),
                    instruction: pc,
                });
            } else {
                errors.push(ValidationError::InvalidOperand {
                    procedure: name.clone(),
                    instruction: pc,
                });
            }
            continue;
        };
        if instruction.stack != effect
            || instruction.variables != variables
            || instruction.effects != effects
        {
            errors.push(ValidationError::InvalidStackContract {
                procedure: name.clone(),
                instruction: pc,
            });
        }
        if let Operand::Branch(target) = instruction.operand
            && target as usize >= count
        {
            errors.push(ValidationError::InvalidBranch {
                procedure: name.clone(),
                instruction: pc,
                target,
            });
        }
        if let Operand::Constant(index) = instruction.operand
            && constants.get(index as usize).is_none_or(|constant| {
                instruction
                    .stack
                    .pushes
                    .first()
                    .is_none_or(|push| push.ty != constant.ty || push.by_ref)
            })
        {
            errors.push(ValidationError::InvalidConstant {
                procedure: name.clone(),
                instruction: pc,
            });
        }
    }
    let mut states: Vec<Option<Vec<StackValue>>> = vec![None; count];
    let mut work = std::collections::VecDeque::new();
    if count > 0 {
        states[0] = Some(Vec::new());
        work.push_back(0usize);
    }
    let mut max_depth = 0usize;
    let mut reaches_end = count == 0;
    while let Some(pc) = work.pop_front() {
        let mut stack = states[pc].clone().unwrap_or_default();
        let instruction = &procedure.instructions[pc];
        let effect = &instruction.stack;
        if stack.len() < effect.pops.len() {
            errors.push(ValidationError::StackUnderflow {
                procedure: name.clone(),
                instruction: pc as u32,
            });
            continue;
        }
        let mut mismatch = false;
        for expected in &effect.pops {
            let actual = stack.pop().expect("stack depth was checked");
            if actual != *expected {
                mismatch = true;
            }
        }
        if mismatch {
            errors.push(ValidationError::StackTypeMismatch {
                procedure: name.clone(),
                instruction: pc as u32,
            });
            continue;
        }
        stack.extend(effect.pushes.iter().cloned());
        max_depth = max_depth.max(stack.len());
        let mut successors = Vec::new();
        match (instruction.opcode, &instruction.operand) {
            (Opcode::ReturnValue, _) | (Opcode::ReturnVoid, _) => {}
            (Opcode::Jump, Operand::Branch(target)) => successors.push(*target as usize),
            (Opcode::JumpIfFalse | Opcode::JumpIfTrue, Operand::Branch(target)) => {
                successors.push(*target as usize);
                successors.push(pc + 1);
            }
            _ => successors.push(pc + 1),
        }
        for next in successors {
            if next >= count {
                reaches_end = true;
                errors.push(ValidationError::MissingReturn {
                    procedure: name.clone(),
                });
            } else if let Some(previous) = &states[next] {
                if previous != &stack {
                    errors.push(ValidationError::InconsistentStackMerge {
                        procedure: name.clone(),
                        instruction: next as u32,
                    });
                }
            } else {
                states[next] = Some(stack.clone());
                work.push_back(next);
            }
        }
        if matches!(instruction.opcode, Opcode::ReturnValue | Opcode::ReturnVoid)
            && !stack.is_empty()
        {
            errors.push(ValidationError::NonEmptyStackAtReturn {
                procedure: name.clone(),
                instruction: pc as u32,
            });
        }
    }
    if reaches_end {
        errors.push(ValidationError::MissingReturn {
            procedure: name.clone(),
        });
    }
    procedure.max_stack = max_depth as u32;
}

fn expected_effect(
    constants: &[Constant],
    module_variables: &[ModuleVariable],
    procedure: &ProcedureCode,
    instruction: &Instruction,
    procedures: &std::collections::HashMap<ProcedureId, ProcedureSignatureView>,
) -> Option<(StackEffect, Vec<VariableRef>, Vec<SideEffect>)> {
    let val = |ty: &TypeInfo| StackValue {
        ty: ty.clone(),
        by_ref: false,
    };
    let reference = |ty: &TypeInfo| StackValue {
        ty: ty.clone(),
        by_ref: true,
    };
    let result = match (instruction.opcode, &instruction.operand) {
        (Opcode::PushConstant, Operand::Constant(index)) => {
            let constant = constants.get(*index as usize)?;
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![val(&constant.ty)],
                },
                vec![],
                vec![],
            )
        }
        (Opcode::LoadLocal, Operand::Variable(VariableRef::Local(index))) => {
            let local = slot(procedure, *index)?;
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![val(&local.ty)],
                },
                vec![VariableRef::Local(*index)],
                vec![SideEffect::ReadVariable],
            )
        }
        (Opcode::StoreLocal, Operand::Variable(VariableRef::Local(index))) => {
            let local = slot(procedure, *index)?;
            (
                StackEffect {
                    pops: vec![val(&local.ty)],
                    pushes: vec![],
                },
                vec![VariableRef::Local(*index)],
                vec![SideEffect::WriteVariable],
            )
        }
        (Opcode::AddressLocal, Operand::Variable(VariableRef::Local(index))) => {
            let local = slot(procedure, *index)?;
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![reference(&local.ty)],
                },
                vec![VariableRef::Local(*index)],
                vec![SideEffect::ReadVariable],
            )
        }
        (Opcode::AddressModule, Operand::Variable(VariableRef::Module(index))) => {
            let variable = module_variables.get(*index as usize)?;
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![reference(&variable.ty)],
                },
                vec![VariableRef::Module(*index)],
                vec![SideEffect::ReadVariable],
            )
        }
        (
            Opcode::ForInitialize,
            Operand::ForLoop {
                counter,
                limit_slot,
                step_slot,
            },
        ) => {
            let counter_type = variable_type(*counter, procedure, module_variables)?;
            if !is_numeric(&counter_type.kind) {
                return None;
            }
            let limit = slot(procedure, *limit_slot)?;
            let step = slot(procedure, *step_slot)?;
            if limit.ty != counter_type || step.ty != counter_type {
                return None;
            }
            (
                StackEffect {
                    pops: vec![val(&counter_type), val(&counter_type), val(&counter_type)],
                    pushes: vec![],
                },
                vec![
                    *counter,
                    VariableRef::Local(*limit_slot),
                    VariableRef::Local(*step_slot),
                ],
                vec![SideEffect::WriteVariable],
            )
        }
        (
            Opcode::ForTest,
            Operand::ForLoop {
                counter,
                limit_slot,
                step_slot,
            },
        ) => {
            let counter_type = variable_type(*counter, procedure, module_variables)?;
            if !is_numeric(&counter_type.kind) {
                return None;
            }
            let limit = slot(procedure, *limit_slot)?;
            let step = slot(procedure, *step_slot)?;
            if limit.ty != counter_type || step.ty != counter_type {
                return None;
            }
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![val(&TypeInfo::boolean())],
                },
                vec![
                    *counter,
                    VariableRef::Local(*limit_slot),
                    VariableRef::Local(*step_slot),
                ],
                vec![SideEffect::ReadVariable],
            )
        }
        (
            Opcode::ForAdvance,
            Operand::ForLoop {
                counter, step_slot, ..
            },
        ) => {
            let counter_type = variable_type(*counter, procedure, module_variables)?;
            if !is_numeric(&counter_type.kind) {
                return None;
            }
            let step = slot(procedure, *step_slot)?;
            if step.ty != counter_type {
                return None;
            }
            (
                StackEffect::default(),
                vec![*counter, VariableRef::Local(*step_slot)],
                vec![
                    SideEffect::ReadVariable,
                    SideEffect::WriteVariable,
                    SideEffect::MayRaise,
                ],
            )
        }
        (Opcode::LoadModule, Operand::Variable(VariableRef::Module(index))) => {
            let variable = module_variables.get(*index as usize)?;
            (
                StackEffect {
                    pops: vec![],
                    pushes: vec![val(&variable.ty)],
                },
                vec![VariableRef::Module(*index)],
                vec![SideEffect::ReadVariable],
            )
        }
        (Opcode::StoreModule, Operand::Variable(VariableRef::Module(index))) => {
            let variable = module_variables.get(*index as usize)?;
            (
                StackEffect {
                    pops: vec![val(&variable.ty)],
                    pushes: vec![],
                },
                vec![VariableRef::Module(*index)],
                vec![SideEffect::WriteVariable],
            )
        }
        (
            Opcode::Unary,
            Operand::Unary {
                operator,
                operand_type,
                result_type,
            },
        ) if supports_unary(operator, operand_type, result_type) => (
            StackEffect {
                pops: vec![val(operand_type)],
                pushes: vec![val(result_type)],
            },
            vec![],
            vec![SideEffect::MayRaise],
        ),
        (
            Opcode::Binary,
            Operand::Binary {
                operator,
                left_type,
                right_type,
                result_type,
            },
        ) if supports_binary(operator, left_type, right_type, result_type) => (
            StackEffect {
                pops: vec![val(right_type), val(left_type)],
                pushes: vec![val(result_type)],
            },
            vec![],
            vec![SideEffect::MayRaise],
        ),
        (Opcode::Convert, Operand::Convert { from, to }) => (
            StackEffect {
                pops: vec![val(from)],
                pushes: vec![val(to)],
            },
            vec![],
            vec![SideEffect::MayRaise],
        ),
        (Opcode::Jump, Operand::Branch(_)) => (
            StackEffect::default(),
            vec![],
            vec![SideEffect::ControlFlow],
        ),
        (Opcode::JumpIfFalse | Opcode::JumpIfTrue, Operand::Branch(_)) => (
            StackEffect {
                pops: vec![val(&TypeInfo::boolean())],
                pushes: vec![],
            },
            vec![],
            vec![SideEffect::ControlFlow],
        ),
        (Opcode::Call, Operand::Call(id)) => {
            let callee = procedures.get(id)?;
            let mut pops = Vec::new();
            for parameter in callee.parameters.iter().rev() {
                pops.push(if parameter.kind == SlotKind::ByRefParameter {
                    reference(&parameter.ty)
                } else {
                    val(&parameter.ty)
                });
            }
            let pushes = callee.return_type.iter().map(val).collect();
            (
                StackEffect { pops, pushes },
                vec![],
                vec![SideEffect::Call, SideEffect::MayRaise],
            )
        }
        (Opcode::Drop, Operand::Drop(ty)) => (
            StackEffect {
                pops: vec![val(ty)],
                pushes: vec![],
            },
            vec![],
            vec![],
        ),
        (Opcode::ReturnValue, Operand::None) if procedure.return_type.is_some() => (
            StackEffect {
                pops: vec![val(procedure.return_type.as_ref()?)],
                pushes: vec![],
            },
            vec![],
            vec![SideEffect::ControlFlow],
        ),
        (Opcode::ReturnVoid, Operand::None) if procedure.return_type.is_none() => (
            StackEffect::default(),
            vec![],
            vec![SideEffect::ControlFlow],
        ),
        _ => return None,
    };
    Some(result)
}

fn slot(procedure: &ProcedureCode, index: u16) -> Option<&VariableSlot> {
    procedure
        .parameters
        .iter()
        .chain(procedure.locals.iter())
        .find(|slot| slot.index == index)
}

fn variable_type(
    variable: VariableRef,
    procedure: &ProcedureCode,
    module_variables: &[ModuleVariable],
) -> Option<TypeInfo> {
    match variable {
        VariableRef::Local(index) => slot(procedure, index).map(|slot| slot.ty.clone()),
        VariableRef::Module(index) => module_variables
            .get(index as usize)
            .map(|slot| slot.ty.clone()),
    }
}
