# Changelog

## Unreleased

- Added opt-in execution for MX `Return <expr>` in functions, loop-targeted
  `Continue For/Do`, and lazy `AndAlso`/`OrElse` Boolean evaluation.
- Continue signals are scoped to the active procedure and matching loop;
  invalid targets produce a runtime error instead of escaping into callers.
- These features remain partial: MX static validation, function return-type
  conversion checks, single-line Continue coverage, and broader legacy
  regression evidence remain outstanding.

### Validation

`cargo check --offline --lib -p vb6parse -p vb6interpret` passed.
`cargo test --offline -p vb6interpret --test run mx_` passed (4 tests);
the full `vb6interpret` suite passed (212 tests), and `vb6parse --lib`
passed (180 tests).
`cargo clippy --offline --lib -p vb6parse -p vb6interpret -- -D warnings`
and `rustfmt --edition 2024 --check` on changed Rust files passed.
The complete interpreter/parser test suites are not yet validated in this
phase; parser integration tests cannot compile because repository fixture
files under `test-data/` are missing.

## Previous Unreleased M3 changes

- Arithmetic between two Currency values now dispatches to checked scaled
  Currency operations for addition, subtraction, multiplication, and division.
  Inexact Currency multiply/divide rescaling is rejected rather than rounded.
- Variant display now formats the new Decimal subtype from its coefficient and
  scale.

### Validation

`cargo test --locked -p vb6interpret` passed (207 unit and integration tests).
Clippy and Rust 2024 formatting checks for changed files passed.
