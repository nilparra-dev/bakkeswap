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
- [x] Port encrypted table writer
- [x] Port read-only package summary/header parsing
- [x] Port Rocket League compressed chunk reader and zlib decompression
- [x] Port NameTable parser (read-only)
- [x] Port ImportTable parser (read-only)
- [x] Port ExportTable parser (read-only)
- [x] Port DependsTable parser (read-only)
- [x] Port target-identity rebuild logic for sandbox output
- [x] Port rebuild validation logic for sandbox output

## Metadata And SQLite

- [x] Add migration runner
- [x] Implement CodeRed import
- [x] Implement local file indexing
- [x] Implement search queries
- [ ] Verify product counts against validated Python import baseline

## Planner And Builder

- [x] Implement TARGET/SOURCE plan resolution
- [x] Persist plans to SQLite and disk
- [x] Emit sandbox-only visual package outputs
- [x] Build visual package outputs from saved plans
- [x] Build thumbnail package outputs when present
- [x] Save validation reports for builds

## Backup And Install Safety

- [x] Implement permanent original backup manager
- [x] Implement per-profile backup manager
- [x] Implement dry-run install preview
- [x] Implement explicit confirmation for real install
- [ ] Implement restore by profile
- [x] Implement original backup verification

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
- [x] Implement `upk known-answer --output`
- [x] Implement `upk rebuild-sandbox`
- [x] Implement `build`
- [x] Implement `install --dry-run`
- [x] Implement `install`
- [ ] Implement `restore --profile`
- [x] Implement `status`
- [x] Implement `backup originals status`
- [x] Implement `backup originals verify`

## Golden Validation

- [x] Synthetic sandbox rebuild tests pass
- [x] Synthetic saved-plan build tests pass
- [x] Synthetic backup manager tests pass
- [ ] Boost known-answer rebuild passes in sandbox
- [ ] Affluenter on Unzipped passes in sandbox
- [ ] Contagion on Standard banner passes in sandbox
- [ ] Cosmological on Bubbly passes in sandbox
- [ ] Lunation on 20XX passes in sandbox
- [ ] Laser Wave III on Gaseous passes in sandbox
- [x] Automated tests never touch real CookedPCConsole
- [x] Sandbox install tests pass

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