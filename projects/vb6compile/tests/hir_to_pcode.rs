use vb6compile::compile_source;
use vb6pcode::{
    DispatchTable,
    program::{FrameSlot, OpcodeIdentity, Operand, PROGRAM_FORMAT_VERSION},
};

#[test]
fn compiles_long_add_to_the_observed_microsoft_opcode_sequence() {
    let source = "Attribute VB_Name = \"AddModule\"\n\
                  Public Function Add(a As Long, b As Long) As Long\n\
                      Add = a + b\n\
                  End Function\n";
    let program = compile_source("AddModule.bas", source).unwrap();

    assert_eq!(program.format_version, PROGRAM_FORMAT_VERSION);
    assert_eq!(program.procedures.len(), 1);
    assert!(program.constants.is_empty());

    let procedure = &program.procedures[0];
    let identities: Vec<_> = procedure
        .instructions
        .iter()
        .map(|instruction| match instruction.opcode {
            OpcodeIdentity::Microsoft(id) => (id.table, id.opcode),
            OpcodeIdentity::MxExtension { .. } => panic!("MX opcode emitted for classic VB6"),
        })
        .collect();
    assert_eq!(
        identities,
        vec![
            (DispatchTable::Primary, 0x6c), // ILdRf
            (DispatchTable::Primary, 0x6c), // ILdRf
            (DispatchTable::Primary, 0xaa), // AddI4
            (DispatchTable::Primary, 0x71), // FStR4
            (DispatchTable::Primary, 0x14), // ExitProc
        ]
    );
    let mnemonics: Vec<_> = procedure
        .instructions
        .iter()
        .map(|instruction| match instruction.opcode {
            OpcodeIdentity::Microsoft(id) => id.catalog_entry().mnemonic,
            OpcodeIdentity::MxExtension { .. } => panic!("MX opcode emitted for classic VB6"),
        })
        .collect();
    assert_eq!(mnemonics, ["ILdRf", "ILdRf", "AddI4", "FStR4", "ExitProc"]);
    assert_eq!(
        procedure.instructions[0].operand,
        Operand::FrameSlot(FrameSlot::Parameter(0))
    );
    assert_eq!(
        procedure.instructions[1].operand,
        Operand::FrameSlot(FrameSlot::Parameter(1))
    );
    assert_eq!(
        procedure.instructions[3].operand,
        Operand::FrameSlot(FrameSlot::FunctionResult)
    );
    assert_eq!(procedure.instructions[2].stack.pops.len(), 2);
    assert_eq!(procedure.instructions[2].stack.pushes.len(), 1);
    assert_eq!(procedure.max_stack, 2);
    assert_eq!(procedure.source_map.len(), procedure.instructions.len());
    assert!(procedure.source_map.iter().all(|entry| entry.line > 0));
}

#[test]
fn diagnoses_add_shapes_without_a_catalogued_lowering() {
    let source = "Attribute VB_Name = \"UnsupportedAdd\"\n\
                  Public Function Add(a As Long, b As Long) As Long\n\
                      Add = a - b\n\
                  End Function\n";
    let errors = compile_source("UnsupportedAdd.bas", source).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.message.contains("binary addition") || error.message.contains("Long addition")
    }));
}
