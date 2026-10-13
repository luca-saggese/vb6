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
- Recognize module-level `As Auto` declarations as unknown-typed rather than
  treating `Auto` as a user-defined type, and report that inference is blocked
  because expression-based inference is not implemented.
- Report `Option Strict` as unsupported: the analyzer does not yet perform
  end-to-end expression, assignment, or call type checking.

### Validation

`cargo test --offline -p vb6semantic` passed (unit, integration, and doc tests).
