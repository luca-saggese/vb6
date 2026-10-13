# vb6pcode

Standalone Microsoft VB6 P-Code catalog and decoder. The crate embeds the six
256-slot tables, operand formats, instruction lengths, stack/x87 and memory
metadata, semantic categories, and the M1 slot classifications. It does not
depend on VB6R's VM, compiler, runtime, or the upstream repository.

The catalog represents Microsoft's P-Code namespace only. Future VB6R/MX
opcodes belong to a separate namespace and must not be added to these six
Microsoft dispatch tables; see the repository's
[MX syntax specification](../../docs/integration/MX-SYNTAX.md).

See [catalog provenance and generation](docs/CATALOG.md) and
[the changelog](CHANGELOG.md).

Run the crate checks offline with:

```sh
cargo fmt --package vb6pcode --check
cargo test --offline -p vb6pcode
cargo clippy --offline -p vb6pcode --all-targets -- -D warnings
```
