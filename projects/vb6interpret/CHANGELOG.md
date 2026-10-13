# Changelog

## Unreleased

- Arithmetic between two Currency values now dispatches to checked scaled
  Currency operations for addition, subtraction, multiplication, and division.
  Inexact Currency multiply/divide rescaling is rejected rather than rounded.
- Variant display now formats the new Decimal subtype from its coefficient and
  scale.

### Validation

`cargo test --locked -p vb6interpret` passed (207 unit and integration tests).
Clippy and Rust 2024 formatting checks for changed files passed.
