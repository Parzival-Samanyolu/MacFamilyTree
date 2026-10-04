# KinTree

A private, offline, open-source family-tree application: a portable **Rust core** (data model, dates in five calendars,
GEDCOM, relationships, data-quality tools, SQLite storage with unlimited undo) and a **React UI** that talks to it through
one JSON command interface. KinTree is original work and is not affiliated with any commercial genealogy product.

> **Status: feature-rich, verified on Linux.** People, events, relationships, GEDCOM / GEDZIP / Gramps / CSV import and export, the
> interactive tree, fan chart, **3D virtual tree**, **maps** (offline gazetteer, heat map, migration, PMTiles), **media library**,
> **stories**, reports (English/Turkish narrative, numbered lists), timeline, calendar, advanced search, find & replace, data-quality
> tools, static website export and password-encrypted backups work and are tested end to end (core line coverage 93%).
> Not done: live sync/sharing, at-rest project encryption, face tagging/OCR, the desktop bundle on macOS/Windows is unverified –
> see [docs/PARITY.md](docs/PARITY.md) for the exact state.

## Try it
```bash
# 1. build the UI
cd ui && npm ci && npm run build && cd ..
# 2. run the Rust core with the web UI on http://127.0.0.1:8787  (loopback only, no authentication)
cargo run --release -p kintree-app --bin kintree-server -- --static ui/dist
```
The desktop shell (`src-tauri`, Tauri 2) uses the same command layer. It needs the platform WebView libraries
(e.g. `libwebkit2gtk-4.1-dev` on Linux) and `cargo install tauri-cli`; build with `cargo tauri build` inside `src-tauri`.
*The desktop bundle has not been built in the environment this code was written in; CI contains the job.*

## Develop
```bash
cargo test --workspace                      # ≈115 Rust tests incl. GEDCOM corpus round-trips
cargo clippy --workspace --all-targets -- -D warnings
cd ui && npm run lint && npm test           # types, lint, unit tests (incl. translation coverage)
cd ui && npm run e2e                        # Playwright against the real Rust core (set CHROMIUM_PATH if needed)
cargo run -p kintree-core --release --example synth -- 100000 > big.ged   # synthetic 100k-person tree
```

## Layout
| Path | What |
|---|---|
| `core/` | pure Rust library: `date`, `name`, `store` (SQLite + undo + FTS5), `gedcom`, `relationship`, `kinship_terms`, `layout`, `quality`, `duplicates`, `stats`, `places` |
| `app/` | `dispatch(session, "area.verb", json)` command API + `kintree-server` HTTP bridge |
| `ui/` | React 18, TypeScript, Vite, Tailwind, Zustand, TanStack Query, i18next (EN/TR), Radix primitives |
| `src-tauri/` | desktop shell |
| `samples/gedcom/` | 15 lossless-round-trip files + 5 torture files |
| `docs/` | [ARCHITECTURE](docs/ARCHITECTURE.md) · [DECISIONS](docs/DECISIONS.md) · [PARITY](docs/PARITY.md) · [PROGRESS](docs/PROGRESS.md) · [PERFORMANCE](docs/PERFORMANCE.md) · [user guides](docs/user/) |

Licenses of dependencies: [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (all permissive).
