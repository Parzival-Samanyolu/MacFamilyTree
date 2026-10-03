# KinTree Architecture

```
/core   kintree-core (Rust lib)  — model, dates, names, places, relationships,
                                   GEDCOM, validation, merge, SQLite storage, undo/redo
/app    Tauri 2 commands (Phase 3) — thin IPC layer over core; no business logic
/ui     React 18 + Vite + TS      — views, canvas/SVG/Three.js renderers, i18n
```

## Layering rules
1. `core` never depends on `app` or `ui`. All logic with a test oracle lives in core.
2. Every mutation is a `Command` executed by `core::command::History`; the UI never writes SQL.
3. `app` exposes coarse commands (`person.update`, `gedcom.import`) returning serde JSON.

## Storage
A project is a folder (zipped as `.ktree`): `project.db` (SQLite, WAL), `media/`, `thumbs/`, `manifest.json`.
Schema is versioned with `PRAGMA user_version`; migrations are an ordered list in `core/src/db/migrations.rs`.
FTS5 virtual table `search_index` is maintained by triggers-free explicit reindex calls inside commands.

## Undo/redo
`history(id, group_id, label, ts, undone)` + `history_ops(history_id, seq, table, pk, before_json, after_json)`.
Executing a command records row images for each touched row; undo applies `before`, redo applies `after`.
`begin_group/end_group` merges several commands into one undo step.

## Date engine
`GenDate` = `{ calendar, qualifier, a: Part, b: Option<Part>, phrase }`. Parsing is locale-tolerant; the sort key
is computed from the earliest instant for the qualifier (before → minus epsilon, after → plus epsilon).

## Key data flow
UI → Tauri command → `core::command` → SQLite → change events → UI query invalidation (TanStack Query).

## Performance plan
Indexed sort keys, prepared statements, bulk import in a single transaction with deferred FK checks,
FTS5 for search, virtualized lists and canvas rendering for trees. Benchmarks go to `docs/PERFORMANCE.md`.

## ER overview
Person 1—* Name; Person 1—* Event; Family *—* Person (partners) ; Family 1—* ChildLink;
Event/Name/ChildLink *—* Citation *—1 Source *—1 Repository; Media *—* any entity (media_link);
Note *—* any entity (note_link); Place self-referencing hierarchy; Task, Association, Tag, Story, Journal standalone.
