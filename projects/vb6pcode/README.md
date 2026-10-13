# vb6pcode

Standalone Microsoft VB6 P-Code catalog and decoder. The crate embeds the six
256-slot tables, operand formats, instruction lengths, stack/x87 and memory
metadata, semantic categories, and the M1 slot classifications. It does not
depend on VB6R's VM, compiler, runtime, or the upstream repository.

The catalog represents Microsoft's P-Code namespace. Future MX instructions
may use only the explicitly versioned extension namespace in the modern
program container; they must not alter these six Microsoft dispatch tables.
See the repository's
[MX syntax specification](../../docs/integration/MX-SYNTAX.md).

The [`program` module](src/program.rs) defines a versioned, platform-independent
container with Microsoft table/slot identities, normalized operands, procedure
metadata, source maps, and stack validation. The current compiler emits only
`ILdRf`, `AddI4`, `FStR4`, and `ExitProc` for the supported `Add(Long, Long)`
HIR shape; see the
[opcode evidence and limits](../../docs/integration/HIR-PCODE.md).

See [catalog provenance and generation](docs/CATALOG.md) and
[the changelog](CHANGELOG.md).

Run the crate checks offline with:

```sh
cargo fmt --package vb6pcode --check
cargo test --offline -p vb6pcode
cargo clippy --offline -p vb6pcode --all-targets -- -D warnings
```
