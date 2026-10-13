# Changelog

## Unreleased

- Add a standalone Microsoft P-Code catalog crate with six generated
  256-entry dispatch tables, full slot/M1 metadata, opcode descriptors, and a
  dependency-free decoder.
- Import the catalog, M1 matrix, evidence-qualified forms, dispatch map,
  Rust table generator, documentation, and required provenance/license files.
- Keep Microsoft dispatch opcodes separate from the future VB6R/MX namespace.
- Add a version-1 canonical VB6R instruction namespace and validated
  in-memory module/procedure representation. Canonical contracts include typed
  stack effects, variable slots, source maps, calls, branches, and counted-loop
  operations; these are not entries in the Microsoft catalog.
- Keep Microsoft opcodes catalogued/decodable distinct from semantically
  verified or emitted opcodes. The HIR compiler emits no Microsoft opcode.
