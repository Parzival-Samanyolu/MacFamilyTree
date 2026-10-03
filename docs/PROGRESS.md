# Progress

## Done
- **Phase 0** — workspace, CI (fmt/clippy/test on 3 OSes), ARCHITECTURE, DECISIONS, BACKLOG, license list.
- **Phase 1 (partial)**
  - `date`: structured `GenDate`; Gregorian, Julian, Hebrew, French Republican, Hijri; qualifiers (abt/est/cal/bef/aft/bet/from/to);
    partial dates, BC, dual-dating, phrase fallback; GEDCOM + EN/TR display; JDN sort keys; ages. Property tests.
  - `name`: name model/formatting, Turkish-aware case & folding, Soundex, Cologne phonetics, edit-distance similarity,
    culture surname inheritance (patrilineal, patronymic, Spanish double, Turkish, Slavic gendered).
  - `store`: SQLite schema v1 for the whole data model, migrations, generic row-image change tracking,
    unlimited undo/redo with grouped transactions and redo invalidation, FTS5 person search (folded, prefix).
  - `model`: typed person/name/family/child/event records, cascading `delete_person` (undoable).

## In progress / next
- Phase 1 remainder: place engine, relationship engine, living-person rule, Double Metaphone + Daitch–Mokotoff,
  coverage measurement (target ≥ 90%), remaining typed records (sources, citations, media, notes, tasks…).
- Phase 2: GEDCOM 5.5/5.5.1/7 import/export, vendor dialects, raw-tag preservation, import log, corpus round-trip tests.

## Blocked
- Nothing. Note: Tauri shell (Phase 3) needs system webkit libs; to be installed in CI/dev environment.

## Not started
Phases 3–12 (UI, tree, charts, media, maps, reports, quality tools, 3D, sync/encryption, packaging).
