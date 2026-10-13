# Changelog

## Unreleased

- Add `compile_source` and `compile_hir` entry points for the
  source → parser → semantic HIR → canonical VB6R P-Code pipeline.
- Add `PCodeBuilder` for typed module constants, globals, procedure signatures,
  parameters, locals, source maps, symbolic labels, calls, assignments,
  expressions, returns, `If`, counted `For`, `Do`, and `While`.
- Require addressable exact-type variables for emitted `ByRef` arguments;
  reject unsupported argument packing, operators, and HIR constructs with
  diagnostics instead of returning partial programs.
- Validate generated programs before returning them, including version,
  procedure/variable/constant tables, branch targets, source maps, declared
  stack contracts, underflow, type compatibility, and branch-join stack state.
- Execution is not implemented or tested; the emitted program is the versioned
  in-memory canonical format consumed by the future VM.

### Validation

`cargo test --offline -p vb6compile --test hir_to_pcode` passes (9 focused
tests), covering shared constants, variables, calls, ByRef, labels, loops,
source maps, rejected HIR, table integrity, and verifier error cases.

The focused P-Code, semantic, and compiler package test suites pass. Clippy
passes for `vb6pcode` and `vb6compile` with `--no-deps`; the dependency-inclusive
Clippy run remains blocked by existing `-D warnings` findings in
`vb6semantic`. Workspace-wide formatting also reports existing changes needed
in `vb6runtime`; formatting checks pass for the changed compiler/P-Code crates.
