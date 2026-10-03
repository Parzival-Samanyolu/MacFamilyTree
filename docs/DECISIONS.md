# Decision Log

| # | Decision | Reason |
|---|----------|--------|
| 1 | Name "KinTree"; no Synium branding/assets. | Legal guardrail from the brief. |
| 2 | Cargo workspace; `core/` (crate `kintree-core`) is a pure library with no Tauri/UI deps. | Core must be unit-testable and portable. |
| 3 | SQLite via `rusqlite` (bundled, FTS5 enabled). | Synchronous, simple, fast; the core is sync, Tauri commands wrap it. |
| 4 | Entity IDs are UUID v4 strings; timestamps are Unix epoch seconds (UTC). | Stable across merge/sync; cheap to index. |
| 5 | Dates are stored as structured JSON-free columns (calendar, qualifier, y/m/d x2, phrase) plus an integer `sort_key` (days since 0000-03-01 proleptic Gregorian). | Indexed range queries and lossless GEDCOM round-trip. |
| 6 | Undo/redo is a per-project command log persisted in SQLite (`history` table). Each command stores forward and inverse SQL-level row operations (before/after row images). | Generic, crash-safe, works for every entity without bespoke inverse logic. |
| 7 | Unknown GEDCOM tags are stored verbatim in `raw_tags` (owner, ordered lines) for lossless re-export. | Brief requirement A. |
| 8 | Dependencies: only MIT/Apache-2.0/BSD-licensed crates; tracked in THIRD_PARTY_LICENSES.md. | Legal guardrail. |
| 9 | Phases are executed in order; UI phases (3+) start only after core gates pass. The Tauri shell is added at Phase 3 because Tauri requires system webkit libs not needed for core work. | Keeps `cargo test` fast and CI green during core phases. |
| 10 | Living-person rule default: no death/burial event and born < 110 years ago (or birth unknown and parents/children living). | Mainstream genealogy-software convention. |
