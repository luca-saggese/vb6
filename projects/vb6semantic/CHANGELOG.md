# Changelog

## Unreleased

- Recognize module-level `As Auto` declarations as unknown-typed rather than
  treating `Auto` as a user-defined type, and report that inference is blocked
  because expression-based inference is not implemented.
- Report `Option Strict` as unsupported: the analyzer does not yet perform
  end-to-end expression, assignment, or call type checking.

### Validation

`cargo check --offline -p vb6semantic` passed.
`cargo test --offline -p vb6semantic --test mx_s04_s08` passed (2 tests).
