# BakkesSwap

BakkesSwap is a local and offline visual item swapper for Rocket League package-backed cosmetics.

This repository is the public Rust + Tauri + SQLite rewrite workspace. It is not a finished release yet.

## What BakkesSwap Is

- a local and offline visual swapper
- a Rust core with a Tauri desktop app target
- a CLI-first rewrite of a proven Python prototype
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

This repository currently contains the rewrite skeleton only.

### Completed

- project skeleton
- Rust workspace layout
- Tauri + Svelte shell
- SQLite schema draft
- CLI contract
- architecture and migration docs
- golden test plan

### Not Yet Implemented

- full `.upk` rebuilder port
- real swap execution
- packaged desktop release
- completed Rust parity with the Python prototype

## Repository Scope

This public repository includes only the safe rewrite workspace:

- Rust/Tauri source skeleton
- Svelte frontend skeleton
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
- `src`: Svelte frontend shell
- `docs`: architecture, schema, CLI, migration, and golden-test docs

## Setup

### Requirements

- Rust stable toolchain
- Node.js and npm
- Tauri prerequisites for your platform

### Suggested Local Setup

1. Install Rust.
2. Install Node.js and npm.
3. From this repository root, run `cargo check`.
4. Run `npm install`.
5. If Tauri prerequisites are installed, run `npm run tauri:dev`.

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
- `docs/SQLITE_SCHEMA.md`
- `docs/CLI_COMMAND_DESIGN.md`
- `docs/MIGRATION_PLAN.md`
- `docs/RUST_PORT_CHECKLIST.md`
- `docs/GOLDEN_TEST_CASES.md`

## Contributing

See `CONTRIBUTING.md` for repository safety rules, development boundaries, and public upload hygiene.