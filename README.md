# BakkesSwap

BakkesSwap is a local and offline visual item swapper for Rocket League package-backed cosmetics.

This repository is the public Rust + Tauri + SQLite rewrite workspace. It now contains a stable Rust backend, a CLI, and the first usable desktop GUI, but it is still a developer-facing rewrite rather than a finished packaged release.

## What BakkesSwap Is

- a local and offline visual swapper
- a Rust core with a Tauri desktop app target
- a CLI-first rewrite of a proven Python prototype
- a Tauri desktop GUI that orchestrates Rust backend services without reimplementing swap logic in the frontend
- focused on the target-identity rebuild method for `.upk`-backed visual swaps

## What BakkesSwap Is Not

- not an item unlocker
- not a server inventory editor
- not an online cheating tool
- not an anti-cheat bypass project
- not a runtime memory editor
- not an injected overlay
- not a Player Title runtime module in v1

Other players will not see your local visual changes unless they independently modify their own local files. BakkesSwap does not modify Rocket League server state.

## Current Status

This repository now contains the stable backend phases through install and restore, plus the first usable Phase 5 Tauri GUI wiring over those backend contracts.

### Completed

- Rust workspace layout and safety-focused project structure
- SQLite migrations, CodeRed import, local file indexing, and status reporting
- product and title search
- TARGET/SOURCE plan creation and saved-plan build output
- install preview, confirmation-gated install execution, restore preview, and confirmation-gated restore execution
- permanent original backup status and verification
- CLI command surface for the implemented backend phases
- Tauri command bridge over `bakkeswap-core`
- Svelte screens for Home, Game Folder, Database, Quick Swap, Install Preview, Active Swaps, Backups, and Logs
- architecture, migration, schema, safety, and phase-status docs

### Not Yet Implemented

- broader golden-answer validation against real known-answer asset cases
- a dedicated settings screen and nonessential desktop polish
- packaged desktop release, signing, updater, and release hardening
- any runtime overlay, Player Title module, online features, or anti-cheat bypass work

## Repository Scope

This public repository includes only the safe rewrite workspace:

- Rust/Tauri source
- Svelte frontend
- SQLite migrations
- architecture and migration docs
- CLI design docs
- golden test case descriptions

This public repository does not include:

- Rocket League game files
- `.upk` assets
- generated modified packages
- personal dumps, backups, or workspace state
- the old Python prototype codebase

The Python prototype exists separately and is not included in this repository.

## Project Layout

- `crates/bakkeswap-core`: core rebuild, validation, database, planner, installer, restore, backups
- `crates/bakkeswap-cli`: CLI entrypoint and command routing
- `src-tauri`: Tauri backend wrapper for the desktop app
- `src`: Svelte desktop GUI and orchestration store
- `docs`: architecture, schema, CLI, migration, and golden-test docs

## Setup

### Requirements

- Rust stable toolchain
- Node.js and npm
- Tauri prerequisites for your platform

### Suggested Local Setup

1. Install Rust.
2. Install Node.js and npm.
3. From this repository root, run `cargo check -p bakkeswap-core -p bakkeswap-cli -p bakkeswap-tauri`.
4. Run `cargo test -p bakkeswap-core`.
5. Run `npm install`.
6. Run `npm run check`.
7. Run `npm run build`.
8. If Tauri prerequisites are installed, run `npm run tauri:dev`.

Running `npm run dev` without Tauri is still useful for frontend-only iteration, but the app will intentionally report that the Rust backend is unavailable and will not perform desktop operations.

## Safety Boundaries

- local and offline visual changes only
- no EAC bypass
- no online cheating
- no server inventory changes
- no runtime memory editing
- no injected overlay in v1
- no Player Title runtime module in v1
- raw rename or raw copy must not be the default build path

## Game Path Examples

Use generic local paths in documentation and tests, such as:

- `C:\Path\To\RocketLeague\TAGame\CookedPCConsole`
- `D:\Games\rocketleague\TAGame\CookedPCConsole`

Do not commit personal machine paths, real game files, inventory dumps, or generated modified packages.

## Documentation

- `docs/ARCHITECTURE.md`
- `docs/BUILDING_FROM_SOURCE.md`
- `docs/SQLITE_SCHEMA.md`
- `docs/CLI_COMMAND_DESIGN.md`
- `docs/MIGRATION_PLAN.md`
- `docs/RUST_PORT_CHECKLIST.md`
- `docs/GOLDEN_TEST_CASES.md`
- `docs/SAFETY_MODEL.md`
- `docs/PHASE_5_TAURI_GUI_STATUS.md`
- `docs/GUI_UX_FLOW.md`

## Contributing

See `CONTRIBUTING.md` for repository safety rules, development boundaries, and public upload hygiene.