# KinTree

Open-source desktop genealogy application (original work; not affiliated with any commercial product).

**Status:** early development — see [docs/PROGRESS.md](docs/PROGRESS.md). The portable Rust core (`core/`) currently provides the
date engine, name/phonetics engine, SQLite storage with unlimited undo/redo and full-text search. UI, GEDCOM, charts, maps,
reports etc. follow the phase plan in [docs/BACKLOG.md](docs/BACKLOG.md).

```
cargo test --workspace      # run core tests
cargo clippy --workspace --all-targets -- -D warnings
```

Docs: [Architecture](docs/ARCHITECTURE.md) · [Decisions](docs/DECISIONS.md) · [Progress](docs/PROGRESS.md) · [Licenses](THIRD_PARTY_LICENSES.md)
