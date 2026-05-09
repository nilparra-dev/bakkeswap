# Rust Port Checklist

## Workspace And Safety

- [x] Create isolated `bakkeswap/` workspace
- [x] Leave Python prototype intact
- [x] Create migration docs
- [x] Create golden test contract
- [x] Create schema draft
- [x] Install Rust toolchain on the build machine
- [x] Run first cargo validation locally

## Core `.upk` Port

- [x] Port encrypted table reader
- [ ] Port encrypted table writer
- [x] Port read-only package summary/header parsing
- [x] Port Rocket League compressed chunk reader and zlib decompression
- [x] Port NameTable parser (read-only)
- [x] Port ImportTable parser (read-only)
- [x] Port ExportTable parser (read-only)
- [x] Port DependsTable parser (read-only)
- [ ] Port target-identity rebuild logic
- [ ] Port rebuild validation logic

## Metadata And SQLite

- [x] Add migration runner
- [x] Implement CodeRed import
- [x] Implement local file indexing
- [x] Implement search queries
- [ ] Verify product counts against validated Python import baseline

## Planner And Builder

- [x] Implement TARGET/SOURCE plan resolution
- [x] Persist plans to SQLite and disk
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
- [x] Implement `selftest`
- [x] Implement `config set-game-path`
- [x] Implement `config show`
- [x] Implement `config validate`
- [x] Implement `db import-codered`
- [x] Implement `db refresh`
- [x] Implement `search`
- [x] Implement `plan`
- [x] Implement `upk inspect`
- [ ] Implement `build`
- [ ] Implement `install --dry-run`
- [ ] Implement `install`
- [ ] Implement `restore --profile`
- [x] Implement `status`
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

- [x] Cargo builds successfully on Windows
- [ ] Tauri bundle builds successfully on Windows
- [ ] Sandbox install tests pass
- [ ] Manual real-install approval checklist exists
- [ ] Offline-only warnings remain prominent
- [ ] No unsafe runtime or online features slipped into v1