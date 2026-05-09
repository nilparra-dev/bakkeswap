# Phase 1 DB/Search Status

Date: 2026-05-09

## Goal

Phase 1 focuses on turning the public Rust/Tauri skeleton into a real database and search foundation without touching the UPK rebuilder, install execution, or runtime modules.

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

## Intentionally Still Stubbed

- UPK rebuilder
- plan execution
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

## Remaining Blockers

### Code validation blocker

- the machine still lacks Windows MSVC/SDK linker libraries required for successful `cargo check`

### Product work still ahead

- full compile validation after Windows build tools are installed
- migration of the real planner/build/install logic
- eventual Tauri wiring to the implemented CLI/core foundation