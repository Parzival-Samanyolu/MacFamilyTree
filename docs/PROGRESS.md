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

- **Phase 2 (core of it)** — `gedcom`: tolerant line-tree parser (CONC/CONT, malformed-line recovery, level clamping),
  charsets (UTF-8/16, ANSEL subset, ISO-8859-1, cp1252), importer (INDI/FAM/SOUR/REPO/NOTE/SNOTE/OBJE, places with
  hierarchy + MAP, associations, PEDI, citations, inline vs record notes/sources/media), verbatim preservation of every
  unknown structure (`raw_tag`) and original xrefs, exporter (5.5.1 and 7.0, UTF-8/UTF-16/Latin-1/ASCII, living-person
  Mask/Exclude, dialect header), import report with line-numbered issues.
  Corpus: 15 clean files (tag-for-tag lossless, re-export byte-identical) + 5 torture files (idempotent).

- **Phase 1/9 logic (core)** — `relationship` (blood/in-law/step relations incl. pedigree collapse, multiple routes, inbreeding &
  relatedness coefficients, Ahnentafel, d'Aboville, Henry, cycle detection), `kinship_terms` (EN, TR, DE, ES, FR, RU, AR;
  Turkish amca/dayı/hala/teyze/yenge/enişte/bacanak/elti/baldız/kayınpeder…), `quality` (21 plausibility rules, severities,
  ignore list, auto-fixes with undo), `duplicates` (blocked scoring, not-a-duplicate memory, person/family merge with full
  rewiring, single undo step), `stats` (all listed statistics with drill-down ids), `synth` generator.
  Undo now restores rows to their exact original position (rowid carried in history images).

- **Phase 4 (Interactive tree)** — `layout` engine (Reingold–Tilford contours; ancestors, descendants, hourglass; multiple spouses;
  collapse; pedigree-collapse stubs; property test for overlaps), SVG renderer with pan/zoom/minimap/keyboard/context menu,
  card designer + saved styles, SVG/PNG export, print.
- **Phase 3 (Basic UI)** — app shell (sidebar, toolbar, status bar, command palette, help, toasts), Dashboard, People (virtualised list),
  person editor (Overview, Names, Events, Relationships, Notes & sources), guided start, import log, export options,
  Settings (language, theme, high contrast, text size, naming convention), file-backed projects + rolling backups.
- **Phase 5 (partial)** — fan chart, relationship calculator, statistics with drill-down.
- **Phase 9** — plausibility checker, duplicates + merge, relationship calculator (UI + core).
- **Quality gates met so far**: `cargo test --workspace` (93 tests), clippy `-D warnings`, rustfmt, ESLint (0 problems), Prettier,
  13 Vitest tests, 18 Playwright E2E tests (incl. axe-core on all main views), `npm audit` 0 vulnerabilities.

- **Phases 6–8 (partial), second pass** — report engine with EN/TR narrative templates and privacy, footnotes and name index, HTML/Markdown
  output; timeline + historical overlay, life-span chart, month calendar, iCalendar export; CSV import/export (EN/TR headers) and JSON export;
  Library (sources with type templates, repositories, tasks, unsourced-facts list).
- **Quality gates now**: 116 Rust tests, clippy `-D warnings`, 13 Vitest, 24 Playwright E2E (axe-core on every main view and on generated reports).
  Bugs found by the new tests and fixed: CSV parent-pair ordering created duplicate families, report HTML lacked `<main>`/`<h1>`, empty table headers,
  low-contrast warning colour, ARIA grid without rows.

## Next (priority order)
1. Media (import, thumbnails, EXIF, face regions, galleries) and a notes browser (Phase 6) — schema exists.
2. Maps (Phase 7) — place engine exists; needs tile source + geocoding strategy.
3. Stories, static website export, DOCX/ODT, more report types (Phase 8).
4. Project encryption, CSV/GEDZIP/Gramps import, JSON/CSV export (Phases 2, 11).
5. Canvas/WebGL renderer for 10k-node trees and a 60 fps measurement; typed bulk-insert path for GEDCOM import speed.
6. Verify the Tauri bundle on macOS/Windows/Linux; add native file dialogs.
7. Coverage tooling and the ≥ 90% core coverage gate.

## Maps (done)
`geo.rs` + `MapView` with tests (7 core, 1 API, 1 E2E incl. axe). Natural Earth 110m countries (public domain) bundled as the offline base map.

## Media (done)
`media.rs` (7 core tests), `media.*` API (1 test), `MediaView` + person Media tab (1 E2E incl. axe). New deps: sha2, image (jpeg/png/gif/webp), kamadak-exif.

## Virtual tree (done)
Three.js scene with LOD labels, 3 themes, accessible list; lazy-loaded chunk (Map and Virtual tree split from the main bundle).

## Stories (done)
`story.rs` (4 tests), API (1 test), `Stories.tsx` (1 E2E incl. axe; the preview iframe is excluded from axe because axe cannot inject into script-less sandboxed frames).

## Archives (done)
`pkg.rs` (ZIP, GEDZIP), `site.rs` (static website), `crypto.rs` (Argon2id + ChaCha20-Poly1305 encrypted project backups; `Store::to_bytes/from_bytes`). 8 core tests, 1 API test, 1 E2E.

## Quality pass (done)
* Core coverage 93.2% lines (target ≥ 90%); CI gate added.
* Bug found by new tests and fixed: undo/redo left the full-text index stale for name and event edits.
* Kinship terms: correct plural agreement (RU/DE/ES/FR), gendered Arabic fallbacks; 7-language matrix test.
* Sidebar digit shortcuts now follow the visible sidebar order; sidebar scrolls.
* Two timing flakes in E2E were test races (read counts / pressed Enter before results rendered) and are fixed.

## Gramps import, merge, line styles (done)
`gramps.rs` (4 tests + API + E2E), merge-import test, divorced-partner lines.

## Known issues
* E2E tests have shown occasional timing races under heavy load (fixed case by case by waiting for rendered results). CI allows one retry.

## Blocked / unverifiable here
* Tauri desktop build (no WebKitGTK in this container). Source + CI job are in place.
* macOS and Windows CI runs (only Linux executed).

## Not started
Phases 6 (except citations/notes), 7, 8, 10, 11 (except backups, i18n and accessibility), 12 (except benchmarks).
