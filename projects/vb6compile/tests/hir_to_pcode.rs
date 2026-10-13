use vb6compile::compile_source;
use vb6pcode::canonical::{self, Opcode, Operand, ValidationError};

#[test]
fn compiles_variables_constants_and_assignments_with_shared_module_pool() {
    let source = "Attribute VB_Name = \"CompileBasics\"\n\
                  Public Sub First()\n\
                      Dim firstValue As Long\n\
                      firstValue = 10\n\
                  End Sub\n\
                  Public Sub Second()\n\
                      Dim secondValue As Long\n\
                      secondValue = 10\n\
                  End Sub\n";
    let mut program = compile_source("CompileBasics.bas", source).unwrap();
    assert_eq!(program.constants.len(), 1);
    assert_eq!(program.procedures.len(), 2);
    assert_eq!(program.procedures[0].locals[0].name, "firstValue");
    assert_eq!(program.procedures[1].locals[0].name, "secondValue");
    assert!(
        program.procedures[0]
            .instructions
            .iter()
            .all(|instruction| instruction.opcode.namespace() == canonical::OpcodeNamespace::Vb6r)
    );
    assert!(program.procedures.iter().all(|procedure| {
        procedure.instructions.len() == procedure.source_map.len() && procedure.max_stack == 1
    }));
    canonical::validate(&mut program).unwrap();
}

#[test]
fn compiles_procedure_calls_returns_and_symbolic_if_branches() {
    let source = "Attribute VB_Name = \"CompileCalls\"\n\
                  Option VB6RExtensions On\n\
                  Public Function Identity(ByVal value As Long) As Long\n\
                      Return value\n\
                  End Function\n\
                  Public Sub Main()\n\
                      Dim result As Long\n\
                      result = Identity(7)\n\
                      If result > 0 Then\n\
                          result = result + 1\n\
                      End If\n\
                  End Sub\n";
    let mut program = compile_source("CompileCalls.bas", source).unwrap();
    assert_eq!(program.procedures.len(), 2);
    let main = &program.procedures[1];
    assert!(
        main.instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::Call)
    );
    assert!(
        main.instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::JumpIfFalse)
    );
    assert!(main.instructions.iter().any(|instruction| {
        instruction.opcode == Opcode::Jump && matches!(instruction.operand, Operand::Branch(_))
    }));
    assert!(
        program.procedures[0]
            .instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::ReturnValue)
    );
    assert!(
        main.source_map
            .iter()
            .all(|entry| entry.line > 0 && entry.column > 0)
    );
    canonical::validate(&mut program).unwrap();
}

#[test]
fn unsupported_control_flow_is_a_diagnostic_not_silent_partial_code() {
    let source = "Attribute VB_Name = \"UnsupportedLoop\"\n\
                  Public Sub Work()\n\
                      For Each item In values\n\
                          item = item\n\
                      Next item\n\
                  End Sub\n";
    let errors = compile_source("UnsupportedLoop.bas", source).unwrap_err();
    assert!(!errors.is_empty(), "For Each must not be silently compiled");
}

#[test]
fn emits_counted_for_do_while_and_continue_branches() {
    let source = "Attribute VB_Name = \"CompileLoops\"\n\
                  Option VB6RExtensions On\n\
                  Public Sub Work()\n\
                      Dim index As Long\n\
                      For index = 1 To 3\n\
                          If index = 2 Then\n\
                              Continue For\n\
                          End If\n\
                      Next index\n\
                      Do While index < 10\n\
                          index = index + 1\n\
                          Continue Do\n\
                      Loop\n\
                      While index > 0\n\
                          index = index - 1\n\
                      Wend\n\
                  End Sub\n";
    let mut program = compile_source("CompileLoops.bas", source).unwrap();
    let procedure = &program.procedures[0];
    assert!(
        procedure
            .instructions
            .iter()
            .any(|instruction| { instruction.opcode == Opcode::ForInitialize })
    );
    assert!(
        procedure
            .instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::ForTest)
    );
    assert!(
        procedure
            .instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::ForAdvance)
    );
    assert!(procedure.instructions.iter().any(|instruction| {
        matches!(instruction.opcode, Opcode::JumpIfFalse | Opcode::JumpIfTrue)
    }));
    canonical::validate(&mut program).unwrap();
}

#[test]
fn emits_typed_byref_address_and_call_contract() {
    let source = "Attribute VB_Name = \"CompileByRef\"\n\
                  Option VB6RExtensions On\n\
                  Public Function Touch(ByRef count As Long) As Long\n\
                      count = count + 1\n\
                      Return count\n\
                  End Function\n\
                  Public Sub Main()\n\
                      Dim count As Long\n\
                      Dim result As Long\n\
                      result = Touch(count)\n\
                  End Sub\n";
    let program = compile_source("CompileByRef.bas", source).unwrap();
    let main = &program.procedures[1];
    assert!(
        main.instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::AddressLocal)
    );
    assert!(
        main.instructions
            .iter()
            .any(|instruction| instruction.opcode == Opcode::Call)
    );
}

#[test]
fn validator_rejects_mutated_stack_contracts_and_branch_targets() {
    let source = "Attribute VB_Name = \"Verifier\"\n\
                  Public Sub Work()\n\
                      Dim value As Long\n\
                      value = 1\n\
                  End Sub\n";
    let mut program = compile_source("Verifier.bas", source).unwrap();
    let push = program.procedures[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.opcode == Opcode::PushConstant)
        .unwrap();
    push.stack.pushes.clear();
    let errors = canonical::validate(&mut program).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::InvalidStackContract { .. }))
    );

    let mut program = compile_source(
        "VerifierBranch.bas",
        "Attribute VB_Name = \"VerifierBranch\"\n\
         Public Sub Work()\n\
             Dim flag As Boolean\n\
             If flag Then\n\
                 flag = True\n\
             End If\n\
         End Sub\n",
    )
    .unwrap();
    let branch = program.procedures[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.opcode == Opcode::JumpIfFalse)
        .unwrap();
    branch.operand = Operand::Branch(u32::MAX);
    let errors = canonical::validate(&mut program).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::InvalidBranch { .. }))
    );
}

#[test]
fn validator_checks_underflow_types_and_branch_stack_convergence() {
    let source = "Attribute VB_Name = \"StackChecks\"\n\
                  Public Sub Work()\n\
                      Dim value As Long\n\
                      value = 1\n\
                  End Sub\n";
    let mut program = compile_source("StackChecks.bas", source).unwrap();
    let push = program.procedures[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.opcode == Opcode::PushConstant)
        .unwrap();
    push.stack.pops.push(vb6pcode::canonical::StackValue {
        ty: vb6core::types::TypeInfo::long(),
        by_ref: false,
    });
    let errors = canonical::validate(&mut program).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::StackUnderflow { .. }))
    );

    let mut program = compile_source("StackTypes.bas", source).unwrap();
    let push = program.procedures[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.opcode == Opcode::PushConstant)
        .unwrap();
    push.stack.pushes[0].ty = vb6core::types::TypeInfo::string();
    let errors = canonical::validate(&mut program).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::StackTypeMismatch { .. }))
    );

    let mut program = compile_source(
        "StackMerge.bas",
        "Attribute VB_Name = \"StackMerge\"\n\
         Public Sub Work()\n\
             Dim flag As Boolean\n\
             If flag Then\n\
                 flag = True\n\
             End If\n\
         End Sub\n",
    )
    .unwrap();
    let jump = program.procedures[0]
        .instructions
        .iter_mut()
        .find(|instruction| instruction.opcode == Opcode::Jump)
        .unwrap();
    jump.stack.pushes.push(vb6pcode::canonical::StackValue {
        ty: vb6core::types::TypeInfo::boolean(),
        by_ref: false,
    });
    let errors = canonical::validate(&mut program).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::InconsistentStackMerge { .. }))
    );
}

#[test]
fn validator_rejects_malformed_procedure_and_slot_tables() {
    let source = "Attribute VB_Name = \"InvalidTables\"\n\
                  Public Sub Work()\n\
                      Dim value As Long\n\
                      value = 1\n\
                  End Sub\n";
    let mut invalid_slot = compile_source("InvalidTables.bas", source).unwrap();
    invalid_slot.procedures[0].locals[0].index = 4;
    assert!(
        canonical::validate(&mut invalid_slot)
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::InvalidVariable { .. }))
    );

    let mut invalid_id = compile_source("InvalidTables.bas", source).unwrap();
    invalid_id.procedures[0].id.0 = 4;
    assert!(
        canonical::validate(&mut invalid_id)
            .unwrap_err()
            .contains(&ValidationError::InvalidProcedureTable)
    );
}

#[test]
fn semantic_failures_do_not_produce_a_program() {
    let errors = compile_source(
        "BadHir.bas",
        "Attribute VB_Name = \"BadHir\"\n\
         Public Function Wrong() As Long\n\
             Return \"not a number\"\n\
         End Function\n",
    )
    .unwrap_err();
    assert!(!errors.is_empty());
}
