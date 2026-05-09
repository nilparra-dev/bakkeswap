# Phase 1 DB/Search Status

Date: 2026-05-09

## Goal

Phase 1 focuses on turning the public Rust/Tauri skeleton into a real database and search foundation without touching the UPK rebuilder, install execution, or runtime modules.

Status: complete and validated.

## Implemented In This Pass

### SQLite foundation

- shared SQLite database service in `bakkeswap-core`
- database file creation and open logic
- automatic schema initialization from the initial migration
- simple settings storage through the `settings` table

### Config path storage

- `config set-game-path <path>` foundation
- `config show` foundation
- `config validate` foundation
- path normalization for:
  - Rocket League root -> `TAGame/CookedPCConsole`
  - `TAGame` -> `CookedPCConsole`
  - `CookedPCConsole` -> unchanged
- helpful invalid-path error generation

### CodeRed metadata import foundation

- `db import-codered <folder>` foundation
- reads safe JSON metadata dumps
- imports products, slots, paints, and titles
- does not require inventory import
- preserves product asset package and thumbnail package metadata fields
- derives candidate `visual_upk` and `thumb_upk` filenames from imported metadata for later planner resolution

### Local file indexing

- scans configured `CookedPCConsole`
- records `.upk` filenames
- preserves exact filename capitalization
- stores file size and SHA-256 hash in `local_files`

### Search

- `bakkeswap search <query>` foundation
- product search by:
  - product ID
  - name
  - slot
  - visual package metadata
  - thumbnail package metadata
- includes title metadata results marked as non-swappable
- JSON output path added with `--json`

### Tests and safe fixtures

- path normalization test coverage
- invalid path handling test coverage
- tiny fake CodeRed dump fixture import test
- search test over imported metadata
- local file index test using temporary runtime-created `.upk` files only

## Validation Result

The Phase 1 validation commands now pass on the current machine:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

## Intentionally Still Stubbed

- UPK rebuilder
- build execution
- real install execution
- restore execution
- backup verification flows beyond status stubs

## Safety State

- no `.upk` files were committed as fixtures
- no Rocket League assets were added
- no personal paths were added to committed fixtures or docs
- no install execution or runtime hooks were added
- offline/local-only boundary remains intact

## Hand-off To Phase 2

Planner work now lives in `PHASE_2_PLANNER_STATUS.md`.

The remaining product work after Phase 1 is the build/install/restore pipeline and the eventual UPK rebuilder port.