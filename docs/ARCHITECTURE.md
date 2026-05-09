# Rust Architecture

## Rewrite Goal

Port the proven Python prototype into a Rust + Tauri + SQLite application without losing the validated target-identity rebuild path, safety boundaries, or install and restore behavior.

## Hard Product Boundaries

- local and offline visual changes only
- no EAC bypass
- no online cheating
- no server inventory changes
- no runtime memory editing
- no injected overlay in v1
- no Player Title runtime module in v1
- raw rename or raw copy must not be the default build method

## Workspace Layout

- `crates/bakkeswap-core`
  - all domain logic
  - `.upk` parsing, table handling, rebuild, validation, database import, planner, builder, installer, restore, backups, path detection, status
- `crates/bakkeswap-cli`
  - first milestone executable
  - command routing and operator-facing outputs
- `src-tauri`
  - desktop shell and Tauri command bridge
  - depends on `bakkeswap-core`
- `src`
  - Svelte frontend shell
  - GUI comes after CLI parity
- `docs`
  - migration, CLI, schema, and golden-test contracts

## Core Module Plan

### `upk`

Rust responsibilities:

- `parser.rs`
- `crypto.rs`
- `name_table.rs`
- `import_table.rs`
- `export_table.rs`
- `depends_table.rs`
- `rebuilder.rs`
- `validator.rs`

Python reference:

- `app/core/_rl_upk_decrypt_tables.py`
- `app/core/_rl_upk_rebuild_identity.py`
- `app/core/rebuilder.py`
- `app/core/validator.py`

### `database`

Rust responsibilities:

- CodeRed import
- refresh pipeline
- search and indexing contract
- SQLite schema ownership

Python reference:

- `app/core/codered_import.py`
- `app/core/db.py`
- `app/core/search.py`

### `services`

Rust responsibilities:

- planner
- builder
- installer
- restore
- permanent original backup manager
- profile backup manager
- game path detection and config
- status tracking

Python reference:

- `app/core/planner.py`
- `app/core/builder.py`
- `app/core/installer.py`
- `app/core/restore.py`
- `app/core/original_backups.py`
- `app/core/paths.py`
- `app/core/status.py`

### `domain`

Rust responsibilities:

- typed records for products, paints, titles, plans, builds, installed swaps, backups, settings, and validation status

Python reference:

- JSON payloads currently built in `_legacy_cli.py`
- app-local data under `app/data` and `workspace/*`

## CLI-First Architecture

The CLI is the first required milestone because the GUI must sit on top of proven build and safety behavior, not recreate it.

Command surface to preserve:

- `bakkeswap selftest`
- `bakkeswap config set-game-path <path>`
- `bakkeswap config show`
- `bakkeswap config validate`
- `bakkeswap db import-codered <folder>`
- `bakkeswap db refresh`
- `bakkeswap search <query>`
- `bakkeswap plan --target <product_id> --source <product_id>`
- `bakkeswap build --plan <plan_path>`
- `bakkeswap install --plan <plan_path> --dry-run`
- `bakkeswap install --plan <plan_path>`
- `bakkeswap restore --profile <profile_name>`
- `bakkeswap status`
- `bakkeswap backup originals status`
- `bakkeswap backup originals verify`

## SQLite Ownership Model

SQLite replaces the current JSON-heavy runtime state for:

- imported metadata
- discovered local file state
- saved swap plans
- build records
- install manifests
- backup tracking
- settings

The database must not hide file-level safety decisions. Dry-run output still needs to show:

- exact files that would be overwritten
- exact backup files that would be created or reused
- the configured CookedPCConsole path used for the operation

## GUI Layer Plan

The Tauri GUI is a thin desktop shell over the Rust CLI-equivalent services.

Planned top-level screens:

- Home
- Game Folder
- Quick Swap
- Active Swaps
- Backups
- Database
- Settings
- Logs

Quick Swap rules:

- TARGET = item the user actually owns and equips
- SOURCE = item the user wants to see locally
- show backup state before install
- show exact files affected before install
- keep explicit confirmation before any real overwrite

## Deferred Overlay Layer

Overlay work is intentionally deferred until after GUI stability.

Allowed future direction:

- external always-on-top window
- F2 hotkey
- no injection
- no memory access

## Non-Negotiable Validation Rules

- automated validation must never modify the real CookedPCConsole
- install tests must target a sandbox copy
- real install remains manual and explicit
- exact filenames and capitalization must be preserved
- safety warnings must remain prominent in both CLI and GUI

## Current Skeleton Verdict

The workspace is ready for Phase 1 implementation work:

- structure exists
- module ownership is defined
- schema draft exists
- CLI contract exists
- GUI shell exists

Implementation parity is not complete yet.