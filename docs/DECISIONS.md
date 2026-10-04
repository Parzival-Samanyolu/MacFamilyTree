# Decision Log

| # | Decision | Reason |
|---|----------|--------|
| 1 | Name "KinTree"; no Synium branding/assets. | Legal guardrail from the brief. |
| 2 | Cargo workspace; `core/` (crate `kintree-core`) is a pure library with no Tauri/UI deps. | Core must be unit-testable and portable. |
| 3 | SQLite via `rusqlite` (bundled, FTS5 enabled). | Synchronous, simple, fast; the core is sync, Tauri commands wrap it. |
| 4 | Entity IDs are UUID v4 strings; timestamps are Unix epoch seconds (UTC). | Stable across merge/sync; cheap to index. |
| 5 | Dates are stored as `date_json` (serialized `GenDate`) plus integer `date_sort` / `date_sort_end` (Julian Day Numbers). | Indexed range queries across all five calendars; lossless GEDCOM round-trip. |
| 6 | Undo/redo is a per-project log in SQLite (`history`, `history_ops`) storing full before/after row images per touched row; recorded in the same SQL transaction as the change (crash-safe). SQLite FK enforcement is off; cascades are explicit, hence undoable. | Generic, crash-safe, works for every entity without bespoke inverse logic. |
| 7 | Unknown GEDCOM tags are stored verbatim in `raw_tags` (owner, ordered lines) for lossless re-export. | Brief requirement A. |
| 8 | Dependencies: only MIT/Apache-2.0/BSD-licensed crates; tracked in THIRD_PARTY_LICENSES.md. | Legal guardrail. |
| 9 | Phases are executed in order; UI phases (3+) start only after core gates pass. The Tauri shell is added at Phase 3 because Tauri requires system webkit libs not needed for core work. | Keeps `cargo test` fast and CI green during core phases. |
| 10 | Living-person rule default: no death/burial event and born < 110 years ago (or birth unknown and parents/children living). | Mainstream genealogy-software convention. |
| 11 | French Republican calendar uses the historical sextile years III, VII, XI, and the arithmetic rule from year XV. | Matches real 1792–1805 records (e.g. 18 Brumaire VIII = 9 Nov 1799). |
| 12 | Hijri uses the tabular civil calendar; may differ by ±1 day from observed (Umm al-Qura) dates. | No offline observational data; documented limitation. |
| 13 | Years are historical-signed (-44 = 44 BC, no year 0); conversion to astronomical happens only in JDN math. | Matches GEDCOM `BC` semantics. |
| 14 | Living rule: manual override wins; death/burial/cremation event → not living; known birth year → living iff within 110 years; no data → living (privacy-safe). Supersedes #10. | Safer default for exports. |
| 15 | GEDCOM export always renumbers nothing: original xrefs are stored and reused so pointers inside preserved raw structures stay valid. | Lossless vendor-extension round trip. |
| 16 | `MAP` coordinates live on the shared place; each event remembers whether its source `PLAC` had a `MAP` (`plac_map`). | Tag-level lossless without duplicating places. |
| 17 | Dialect option currently changes only the header SOUR identification. Vendor-specific data is preserved verbatim rather than translated. | Honest scope; translation tables are backlog. |
| 18 | Maps: MapLibre GL JS with a bundled Natural Earth country outline as the default offline base map; OSM raster tiles only when the user selects them; local PMTiles via the `pmtiles` protocol (file picker, no server). Worker files are copied unhashed to `/maplibre/` by a Vite plugin because the worker imports a sibling module by name. | Offline-first with zero network by default; privacy (tile requests reveal the viewed area). |
| 19 | Geocoding: offline gazetteer embedded in the core (`core/data/gazetteer.csv`); online Nominatim is called from the UI only after an explicit opt-in, throttled to 1 req/s and cached, so the core stays network-free. Places inherit the nearest ancestor's coordinates for display (flagged approximate). | Honours the Nominatim usage policy and keeps the core deterministic and testable. |
| 20 | Media bytes are stored in the project database (`media_blob`, schema v2) outside the undo log; deleting a media row keeps its blob until `media.purge`, so undo can always restore it. Linked-only items (from GEDCOM) can be relinked by attaching a file. | One self-contained, portable, backup-friendly file; undo correctness without copying blobs into history. |
