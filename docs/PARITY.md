# Feature-parity checklist

Maps every section of the original requirements to its status. ✅ done and tested · 🟡 partial · ⬜ not started.
"Tests" names where the behaviour is proven. This file is the source of truth for what the application can do **today**.

## A. Project management
| Item | Status | Code / tests |
|---|---|---|
| New / open / close project, autosave, crash-safe journal | ✅ | `core/store.rs` (SQLite WAL); `app/tests/api_tests.rs::file_backed_projects_*`; E2E "file-backed project" |
| Duplicate project / manual backup / restore | ✅ | `project.backup`, `project.restore`; same tests |
| Automatic rolling backups | ✅ | `rolling_backup` (keeps 10) |
| Recent projects list, lock screen | ⬜ | |
| Password protection / AES-256-GCM + Argon2id | ⬜ | planned Phase 11 |
| Import GEDCOM 5.5 / 5.5.1 / 7.0 | ✅ | `core/gedcom/import.rs`; `core/tests/gedcom_roundtrip.rs` |
| Vendor extensions (`_UID`, `_PLAC`, `_MILT`, `_FSFTID`, …) | 🟡 | preserved verbatim and re-exported; not interpreted (e.g. `_MARNM` is kept, not turned into a married name) |
| Lossless unknown tags | ✅ | `raw_tag` table; 15 clean files re-export tag-for-tag |
| Character sets UTF-8 / UTF-16 / ANSEL / ASCII / ISO-8859-1 | 🟡 | `gedcom/charset.rs`; ANSEL covers common diacritics and letters, not the full table |
| Malformed input handled with detailed log | ✅ | torture corpus; `malformed_file_reports_detailed_issues`; E2E import-log test |
| GEDZIP, CSV, Gramps XML import | ⬜ | |
| Export GEDCOM 5.5.1 / 7.0, living privacy, charset | ✅ | `gedcom/export.rs`; `living_privacy_mask_and_exclude`, `gedcom7_export_*` |
| Target-software dialect | 🟡 | header identification only |
| Export CSV / JSON / PDF / HTML website | ⬜ | |
| Round-trip ≥ 15 files incl. torture | ✅ | `samples/gedcom/clean` (15) + `torture` (5) |
| Merge two projects | ⬜ | person/family/place merge exist; whole-project merge does not |

## B. Navigation
| View | Status | Notes |
|---|---|---|
| Dashboard | ✅ | on this day, upcoming birthdays, quality score, favorites, recent, random person |
| Persons (list + inspector) | ✅ | virtualised; 2,000-person E2E |
| Families | 🟡 | edited inside the person's Relationships tab; no standalone list |
| Tree (interactive) | ✅ | see D |
| Virtual Tree (3D), Maps, Timeline, Media, Repositories, Stories, Tasks, Reports, Calendar | ⬜ | |
| Sources, Notes | 🟡 | add/remove citations and notes on a person; no standalone browsers |
| Charts | 🟡 | see E |
| Statistics, Duplicates, Plausibility, Settings | ✅ | |
| Global search / command palette (Ctrl/Cmd-K) | ✅ | E2E palette test |
| Back / forward history | ✅ | Alt+←/→ |
| Multiple windows | ⬜ | |

## C. Person editing
| Item | Status | Notes |
|---|---|---|
| Tabs: Overview, Names, Events & facts, Relationships, Notes & sources | ✅ | |
| Tabs: Associations, Media, Tasks, Custom fields, History | ⬜ | associations exist in the data model and GEDCOM `ASSO` |
| Add father / mother / partner / child / sibling with surname rules (patrilineal, patronymic, Spanish double, Turkish, Slavic) | ✅ | `name.rs`; `naming_cultures_for_children`, `blank_surname_means_inherit_not_empty` |
| Drag-and-drop reorder of events and children | ✅ | partners/media: ⬜ |
| Autocomplete for places | ✅ | names / occupations / sources: ⬜ |
| Bulk edit, find/replace | ⬜ | |
| Age at event | ✅ | `event_json` |
| Ahnentafel, d'Aboville, Henry numbering | 🟡 | core + `chart.numbering` API; no UI. Register: ⬜ |
| Living detection and privacy | ✅ | `model::living_ids`; masked/excluded in GEDCOM export. Reports: ⬜ |

## D. Interactive tree
| Item | Status | Notes |
|---|---|---|
| Pan / zoom / minimap / fit | ✅ | `treeGeom.test.ts`, E2E |
| Hourglass, ancestors, descendants; top-down and left-right | ✅ | `core/layout.rs` (Reingold–Tilford contours); `layout_tests.rs` (no-overlap property test) |
| Fan-like compact, "all relatives" layout | ⬜ | |
| Multiple spouses, collapse/expand, pedigree-collapse stubs + jump | ✅ | |
| Card designer (fields, size, colour by sex/generation/custom, rounded), saved styles | ✅ | |
| Context menu, keyboard navigation, set root | ✅ | E2E |
| Line styles by relationship | 🟡 | dashed for adopted/foster/step; divorced ⬜ |
| Export SVG / PNG, print | ✅ | PDF poster tiling, A0 plotter: ⬜ |
| 60 fps at 10,000 visible nodes | ⬜ | SVG with viewport culling; Canvas/WebGL renderer not built; not measured |

## E. Charts
Ancestor ✅, Descendant ✅, Hourglass ✅, Fan (360/180/90°) ✅, statistics charts ✅ (drill-down). Relationship **calculator** ✅ (no drawn chart).
Bow-tie, family-group sheet, generation/kinship chart, timeline chart, calendar chart, migration chart ⬜. PDF export ⬜ (SVG/PNG/print only).

## F–H. Virtual tree, Maps, Timeline & Calendar
⬜ Not started. Place engine exists (hierarchy, find-or-create, merge, duplicates — `core/places.rs`); no map UI, geocoding or KML/GeoJSON.

## I. Statistics ✅
All listed measures except "average lifespan by decade" (century only) — `core/stats.rs`, `stats_tests.rs`, E2E drill-down.

## J–L. Reports, Stories, Media ⬜
Schema tables exist (`story`, `media`, `media_link`); no UI or generators.

## M. Sources, citations, repositories 🟡
Data model, GEDCOM round trip and add/remove citation + source on persons ✅. Templates, bibliography, repository UI, "facts without sources" view ⬜ (source coverage % is in Statistics).

## N. Search 🟡
FTS5 prefix search, Turkish/diacritic folding ✅. Soundex and Cologne phonetics used by duplicate detection (not exposed in search). Double Metaphone, Daitch–Mokotoff, query builder, saved searches ⬜.

## O. Data quality ✅ (🟡 for extras)
21 plausibility rules with severities, ignore list, auto-fix + undo; duplicate scoring, merge with full rewiring and undo, "not a duplicate" memory; orphan/dangling-reference checks. Place cleanup is in core only. Split project by branch ⬜.

## P. Relationship calculator ✅
Blood (full/half, multiple routes, pedigree collapse), in-law, step, adoptive; inbreeding and relatedness coefficients; 7 languages incl. Turkish amca/dayı/hala/teyze/yenge/enişte/elti/bacanak/baldız/kayınpeder — `relationship_tests.rs`, E2E.

## Q. Sync, sharing, publishing ⬜

## R. Productivity & polish
Dashboard ✅ (home person ⬜) · Onboarding wizard ✅ · Sample projects (30 / 2,000; 100,000 via generator) ✅ · Preferences 🟡 (language, theme, high contrast, text size, naming convention; no shortcut customisation) · Shortcut map + command palette ✅ · Help 🟡 (shortcut sheet only; no searchable manual) · No telemetry ✅.

## Cross-cutting requirements
| Requirement | Status |
|---|---|
| Layered `core` → `app` → `ui`, core testable without UI | ✅ |
| Unlimited, grouped undo/redo for every mutation, restoring exact row positions | ✅ |
| Open 100k-person project < 3 s | ✅ (≈1 ms) — `docs/PERFORMANCE.md` |
| Search < 100 ms at 100k | ✅ (≈2 ms) |
| GEDCOM import speed | 🟡 100k persons in ≈43 s (linear; optimisation planned) |
| Accessibility: keyboard, labels, high contrast, scalable text, dark/light | ✅ axe-core scans of every main view (light theme) pass in CI |
| i18n EN + TR, all strings externalised | ✅ key-coverage test |
| Core unit-test coverage ≥ 90% | ⬜ not measured (93 Rust tests; coverage tooling not set up) |
| CI on 3 OSes | 🟡 workflow written; only the Linux run has been executed here |
| Desktop packaging (.dmg/.msi/AppImage/.deb) | 🟡 Tauri 2 shell source + CI job written; **not built in this environment** (needs system WebView libs) |
| Canvas/WebGL, Three.js, MapLibre, ONNX face detection, OCR | ⬜ |
