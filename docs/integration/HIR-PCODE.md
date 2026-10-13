# Typed HIR to canonical VB6R P-Code

`vb6compile::compile_source` runs `vb6parse`, `vb6semantic`, the HIR compiler,
and the canonical P-Code validator. `compile_hir` accepts only a compilable
`HirAnalysis`; an absent HIR, semantic errors, or unsupported forms produce
source-located diagnostics instead of partial output.

The compiler does **not** emit Microsoft dispatch opcodes. `vb6pcode`'s six
Microsoft tables remain a catalog and decoder:

| Classification | Meaning |
|---|---|
| Catalogued | A slot/form and M1 metadata exist in the Microsoft tables. |
| Decodable | `decode_one` can decode bytes matching the recorded length and operand format. |
| Semantically verified for the compiler | None of the Microsoft opcodes in this tranche. Catalog and decoder metadata alone do not establish VM behavior. |
| Emitted | None. The compiler emits only the separate, versioned VB6R namespace. |

Canonical VB6R instructions have IDs in the `0x8000` range and format version
`1`; those IDs are not Microsoft opcode identifiers. Each instruction records
its operand contract, stack pops/pushes, variable references, side effects,
and source-map index. The validator checks these contracts, constant and slot
indices, procedure/call tables, branch targets, stack underflow and type
compatibility, and stack-state equality at control-flow joins. Branches use
resolved procedure-local instruction indices. Module constants are shared by
all procedures and references are checked against the pool.

## Current emission coverage

| HIR/source construct | Status | Notes |
|---|---|---|
| Numeric/string/Boolean and `Nothing`/`Null`/`Empty` literals | PARTIAL | Typed module constant pool; numeric encodings outside the implemented literal forms diagnose. |
| Local/module variables and assignments | PARTIAL | Typed slots and source symbols; constant declarations lack initializer values in HIR and diagnose. |
| Parentheses and explicit HIR conversions | PARTIAL | Conversion is a checked canonical instruction; VM execution is not part of this change. |
| Unary `+`/`-`, typed arithmetic/comparison, string `&` | PARTIAL | Only canonical operator/type combinations are admitted; every other operator is diagnosed. |
| Direct same-module Function calls | PARTIAL | Arity and parameter metadata are retained; `ByRef` requires an addressable exact-type variable. `Optional` omission and `ParamArray` packing diagnose. |
| `ByVal` / `ByRef` parameters | PARTIAL | Frame metadata and address/value stack contracts are emitted. No runtime execution is claimed. |
| `Return <expr>`, Function-result assignment, Sub fallthrough | PARTIAL | Explicit return values and the implicit result slot are represented. |
| `If` | PARTIAL | Symbolic labels resolve to instruction indices; join stacks are verified. |
| Numeric `For`, `Continue For`, `Do`, `While`, `Continue Do` | PARTIAL | Dedicated canonical loop operations retain typed bounds, limit/step slots, signed-step inclusive checks, and loop target labels. |
| `For Each`, `Select Case`, arrays, objects, properties/events, `Try`, lazy MX, lambdas, spread | BLOCKED | HIR currently cannot provide a complete compilable representation for these forms, or no canonical instruction contract is emitted. |

The canonical program is a validated in-memory representation, not a serialized
Microsoft P-Code byte stream. No `vb6vm` execution or debugger integration is
claimed. In particular, tests here validate emitted structure and verifier
behavior only; execution semantics must be tested against the future VM.
