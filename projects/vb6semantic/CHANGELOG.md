# Changelog

## Unreleased

- Add a typed HIR API for VB6 module procedures, parameter signatures,
  declarations, literals, names, arithmetic expressions, and assignments.
  HIR retains CST byte spans and stable scope/name symbol identities; semantic
  errors and statements not yet lowered prevent a compilable program result.
- Lower structured `If`, `While`, `Do`, and counted `For` control flow, and bind
  typed calls with arity and argument compatibility checks.
- Resolve VB6 symbol names case-insensitively during scope lookup and duplicate
  declaration checks.
- Document the current VB6 HIR lowering boundary and reserved, unimplemented
  MX HIR forms.
- Add opt-in `As Auto` inference for supported numeric, string, Boolean, and
  `Nothing` literal initializers. Missing/unsupported initializers, parameters,
  and use without the MX option produce explicit diagnostics; legacy untyped
  declarations remain `Variant`.
- Add partial `Option Strict On` HIR checks for known typed assignments.
  Differing types are rejected; `Variant`, unknown, call/member expressions,
  and other unsupported cases are diagnosed rather than silently accepted.
- Preserve distinct HIR types for `Nothing`, `Null`, and `Empty`.

### Validation

`cargo test --offline -p vb6semantic --test mx_s04_s08` passed (6 tests);
`cargo test --offline -p vb6semantic` passed (unit, integration, and doc tests).
