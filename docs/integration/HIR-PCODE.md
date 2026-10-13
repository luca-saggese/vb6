# Minimal HIR → Microsoft P-Code: `Add(Long, Long)`

The current compiler tranche accepts this typed-HIR shape only:

```vb
Function Add(a As Long, b As Long) As Long
    Add = a + b
End Function
```

It emits Microsoft dispatch-table identities in the modern `vb6pcode::program`
container. There is no duplicate set of VB6-classic VB6R opcodes. No MX opcode
is emitted; the container reserves a separately versioned extension namespace.

## Emitted opcodes

All identities below are unprefixed Primary-table slots. The old `vb6-r`
disassembly report records the sample stream as
`6C 0C 00 6C 10 00 AA 71 78 FF 14 00` and documents handlers in the original
`MSVBVM60.DLL` whose SHA-256 is
`e043c59c66ba2dc549bf5ac104c70279ec05931d531de2971458eab91871f7b6`.
The compiler does not copy that sample's x86 frame displacements into its
program; it uses typed symbolic frame slots instead.

| Order | Microsoft identity | Encoded form in Microsoft P-Code | Stack effect | Lowered meaning and evidence |
|---:|---|---|---|---|
| 1 | Primary `0x6C` `ILdRf` | `%a`, 3 bytes | 0 → 1 DWORD | Load parameter slot `a`. Catalog: [`opcodes.csv`](../../projects/vb6pcode/data/opcodes.csv), row 205. M4 handler report: [`WS-A-HANDLERS.md`](../../../vb6-r/research/m5/pcode/WS-A-HANDLERS.md), lines 24–40. |
| 2 | Primary `0x6C` `ILdRf` | `%a`, 3 bytes | 0 → 1 DWORD | Load parameter slot `b`; same handler contract and sources as above. |
| 3 | Primary `0xAA` `AddI4` | no operand, 1 byte | 2 DWORDs → 1 DWORD | Signed 32-bit addition with overflow path. Catalog: [`opcodes.csv`](../../projects/vb6pcode/data/opcodes.csv), row 267. M4 handler report: [`WS-A-HANDLERS.md`](../../../vb6-r/research/m5/pcode/WS-A-HANDLERS.md), lines 24–40. |
| 4 | Primary `0x71` `FStR4` | `%a`, 3 bytes | 1 DWORD → 0 | Store the result in the symbolic Function-result slot. Catalog: [`opcodes.csv`](../../projects/vb6pcode/data/opcodes.csv), row 210. M4 handler report: [`WS-A-HANDLERS.md`](../../../vb6-r/research/m5/pcode/WS-A-HANDLERS.md), lines 24–40. |
| 5 | Primary `0x14` `ExitProc` | no operand, 1 byte | 0 → 0 | Return from the Function after its result slot has been written. Catalog: [`opcodes.csv`](../../projects/vb6pcode/data/opcodes.csv), row 117. M4 handler report: [`WS-A-HANDLERS.md`](../../../vb6-r/research/m5/pcode/WS-A-HANDLERS.md), lines 24–40. |

The operand forms and primary/lead-byte distinction are described in the
[Avast P-Code disassembly article](https://www.gendigital.com/blog/insights/research/vb6-p-code-disassembly?utm_source=chatgpt.com).
The compiler's normalized operands are `Parameter(0)`, `Parameter(1)`, and
`FunctionResult`; they are not `%a` x86 displacements or PE/COM addresses.

## Evidence boundaries

- The opcode identities, names, operand formats, byte lengths, and table stack
  metadata are catalog observations. The M4 report cites disassembled handler
  behavior for this exact DLL hash: `ILdRf` reads a DWORD from the frame,
  `AddI4` adds two DWORDs with an overflow path, `FStR4` stores a DWORD to the
  frame, and `ExitProc` performs procedure exit/cleanup.
- In the old M4 implementation, these are metadata-only opcode descriptors;
  the M4 golden test executes profile-specific IR, not these Microsoft
  handlers. The reported opcode execution path is limited to a build-bound
  `F5/LitI4` case.
- The documented byte stream does not independently prove that source-level
  `Function Add(a As Long, b As Long)` maps to those exact frame positions.
- The test here validates the HIR-to-opcode identities, normalized operands,
  source map, and stack contracts. It does not execute P-Code or claim full
  Microsoft runtime compatibility.
- The previous general VB6R opcode set and broad lowering are removed. Other
  constructs currently produce diagnostics; this tranche does not lower
  branches, calls, loops, conversions, or MX syntax.
