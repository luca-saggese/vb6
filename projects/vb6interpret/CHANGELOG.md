# Changelog

## Unreleased

### Blocked
- MX S06 (`Try`/`Catch`/`Finally`) and S07 (`Using`) remain **BLOCKED**.
  The interpreter dispatches directly over the CST and has no Try/Using
  control-flow nodes or cleanup-unwinding mechanism; adding syntax without
  guaranteed `Finally` execution on every flow and error exit would be unsafe.
  The runtime also lacks a `Dispose` protocol for `Using`.

- Added opt-in execution for MX `Return <expr>` in functions, loop-targeted
  `Continue For/Do`, and lazy `AndAlso`/`OrElse` Boolean evaluation.
- Added opt-in interpolation for `$"..."`, including brace escaping and
  expression evaluation in the current interpreter scope.
- Added MX `??` evaluation: only `Nothing` selects the fallback; `Null` and
  `Empty` remain distinct and are returned unchanged. Added literal mapping for
  those three VB sentinel values, required to execute the coalescing expression.
- Added partial MX array-literal evaluation for non-empty rank-1, zero-based
  arrays with supported common element types. Empty literals explicitly error
  because the interpreter has no expected-type context.
- Routed single-line `If ... Then Continue For/Do` through the existing
  MX-gated continue parser and interpreter flow propagation.
- Continue signals are scoped to the active procedure and matching loop;
  invalid targets produce a runtime error instead of escaping into callers.
- These features remain partial: MX static validation, function return-type
  conversion checks, single-line Continue coverage, and broader legacy
  regression evidence remain outstanding.

### Validation

`cargo check --offline --lib -p vb6parse -p vb6interpret` passed.
`cargo test --offline -p vb6interpret --test run mx_` passed (7 tests);
`mx_array_literal` passed (4 tests), `mx_inline_continue` passed (3 tests),
and the full `vb6interpret` suite passed (222 tests); `vb6parse --lib`
passed (181 tests), and `cargo test --offline -p vb6semantic --test
mx_s04_s08` passed (2 tests).
`cargo clippy --offline --lib -p vb6parse -p vb6semantic -p vb6interpret -- -D warnings`
and `rustfmt --edition 2024 --check` on changed Rust files passed.
Parser integration tests remain unavailable because repository fixture files
under `test-data/` are missing.

## Previous Unreleased M3 changes

- Arithmetic between two Currency values now dispatches to checked scaled
  Currency operations for addition, subtraction, multiplication, and division.
  Inexact Currency multiply/divide rescaling is rejected rather than rounded.
- Variant display now formats the new Decimal subtype from its coefficient and
  scale.

### Validation

`cargo test --locked -p vb6interpret` passed (207 unit and integration tests).
Clippy and Rust 2024 formatting checks for changed files passed.
