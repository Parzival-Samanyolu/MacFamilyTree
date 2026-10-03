# Performance record

Reproduce: `cargo test -p kintree-core --release --test bench -- --ignored --nocapture`
(synthetic trees from `kintree_core::synth`, seed 42; Linux x86-64 container, release build, file-backed SQLite/WAL).

| Persons | Events | GEDCOM | Import | Open existing project | Search ("yilmaz ali") | Export GEDCOM |
|---|---|---|---|---|---|---|
| 30 | 79 | 6 KB | 18 ms | 0.9 ms | 0.2 ms | 3 ms |
| 2,000 | 4,453 | 397 KB | 654 ms | 0.8 ms | 0.3 ms | 122 ms |
| 100,000 | 185,877 | 18.8 MB | 42.6 s | 0.8 ms | 2.0 ms | 13.2 s |

## Targets
| Target | Status |
|---|---|
| Open a 100,000-person tree < 3 s | **Met** (≈1 ms; opening reads no rows — lists/trees are loaded lazily). |
| Search results < 100 ms | **Met** (2 ms at 100k, FTS5 prefix search). |
| 60 fps pan/zoom at 10,000 visible nodes | Not yet measured (UI phase). |

## Known weak spot
GEDCOM import/export are linear but heavy (~0.4 ms/person import, ~0.13 ms/person export). Profile (callgrind) shows time is
spread over SQLite writes (`INSERT OR REPLACE` with indexes) and per-row `serde_json::Map` building, with no single hotspot.
Planned fix (backlog P12): typed bulk-insert path with prepared statements and deferred index creation, and typed row structs
for export instead of JSON maps. Already fixed: quadratic FTS deletes (import of 2k persons fell from >1 s of reindexing to linear),
N+1 reads in `Store::rows`, uncached prepared statements, duplicate event writes.
