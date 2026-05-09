# Rust Port Checklist

## Workspace And Safety

- [x] Create isolated `bakkeswap/` workspace
- [x] Leave Python prototype intact
- [x] Create migration docs
- [x] Create golden test contract
- [x] Create schema draft
- [ ] Install Rust toolchain on the build machine
- [ ] Run first cargo validation locally

## Core `.upk` Port

- [ ] Port encrypted table reader
- [ ] Port encrypted table writer
- [ ] Port NameTable parser and writer
- [ ] Port ImportTable parser and writer
- [ ] Port ExportTable parser and writer
- [ ] Port DependsTable parser and writer
- [ ] Port target-identity rebuild logic
- [ ] Port rebuild validation logic

## Metadata And SQLite

- [ ] Add migration runner
- [ ] Implement CodeRed import
- [ ] Implement local file indexing
- [ ] Implement search queries
- [ ] Verify product counts against validated Python import baseline

## Planner And Builder

- [ ] Implement TARGET/SOURCE plan resolution
- [ ] Persist plans to SQLite and disk
- [ ] Build visual package outputs
- [ ] Build thumbnail package outputs when present
- [ ] Save validation reports for builds

## Backup And Install Safety

- [ ] Implement permanent original backup manager
- [ ] Implement per-profile backup manager
- [ ] Implement dry-run install preview
- [ ] Implement explicit confirmation for real install
- [ ] Implement restore by profile
- [ ] Implement original backup verification

## CLI Parity

- [x] Define command surface skeleton
- [ ] Implement `selftest`
- [ ] Implement `config set-game-path`
- [ ] Implement `config show`
- [ ] Implement `config validate`
- [ ] Implement `db import-codered`
- [ ] Implement `db refresh`
- [ ] Implement `search`
- [ ] Implement `plan`
- [ ] Implement `build`
- [ ] Implement `install --dry-run`
- [ ] Implement `install`
- [ ] Implement `restore --profile`
- [ ] Implement `status`
- [ ] Implement `backup originals status`
- [ ] Implement `backup originals verify`

## Golden Validation

- [ ] Boost known-answer rebuild passes in sandbox
- [ ] Affluenter on Unzipped passes in sandbox
- [ ] Contagion on Standard banner passes in sandbox
- [ ] Cosmological on Bubbly passes in sandbox
- [ ] Lunation on 20XX passes in sandbox
- [ ] Laser Wave III on Gaseous passes in sandbox
- [ ] Automated tests never touch real CookedPCConsole

## GUI

- [x] Create Svelte shell
- [x] Create Tauri backend shell
- [ ] Connect GUI to Rust command layer
- [ ] Implement Home screen
- [ ] Implement Game Folder screen
- [ ] Implement Quick Swap screen
- [ ] Implement Active Swaps screen
- [ ] Implement Backups screen
- [ ] Implement Database screen
- [ ] Implement Settings screen
- [ ] Implement Logs screen

## Release Readiness

- [ ] Cargo builds successfully on Windows
- [ ] Tauri bundle builds successfully on Windows
- [ ] Sandbox install tests pass
- [ ] Manual real-install approval checklist exists
- [ ] Offline-only warnings remain prominent
- [ ] No unsafe runtime or online features slipped into v1