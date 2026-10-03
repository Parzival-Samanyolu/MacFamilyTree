# Contributing

1. **Logic belongs in `core/`** and must be unit-testable without a UI. `app/` only translates JSON to core calls. The UI never builds SQL or computes genealogy.
2. **Every mutation goes through `Store::transact`** so it is undoable. Use `Tx::put_row` / `Tx::delete`; clean up references explicitly (SQLite foreign keys are intentionally off so cascades are visible to undo).
3. **Tests**: new core behaviour needs a unit/integration test; new user-visible behaviour needs a Playwright test. GEDCOM changes must keep `core/tests/gedcom_roundtrip.rs` green – add a file to `samples/gedcom/` for new structures.
4. **UI strings** go in `ui/src/i18n/en.json` *and* `tr.json`; the unit test fails when they diverge or a used key is missing.
5. **Accessibility**: new views must pass the axe scan in `ui/e2e/app.spec.ts`; every control needs an accessible name; don't nest interactive elements.
6. **Before pushing**: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and in `ui/`: `npm run lint && npm run format:check && npm test && npm run e2e`.
7. **Legal**: no third-party branding, artwork, code or data. Only permissively licensed dependencies; regenerate `THIRD_PARTY_LICENSES.md` with `python3 scripts/gen_licenses.py`.
8. Record non-obvious decisions in `docs/DECISIONS.md`.
