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
- [x] Implement restore by profile
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
- [x] Implement `restore --profile`
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
- [x] Sandbox restore tests pass

## GUI

- [x] Create Svelte shell
- [x] Create Tauri backend shell
- [x] Connect GUI to Rust command layer
- [x] Implement Home screen
- [x] Implement Game Folder screen
- [x] Implement Quick Swap screen
- [x] Implement Install Preview screen
- [x] Implement Active Swaps screen
- [x] Implement Backups screen
- [x] Implement Database screen
- [ ] Implement Settings screen
- [x] Implement Logs screen

## Release Readiness

- [x] Cargo builds successfully on Windows
- [ ] Tauri bundle builds successfully on Windows
- [ ] Sandbox install tests pass
- [ ] Manual real-install approval checklist exists
- [x] Offline-only warnings remain prominent
- [ ] No unsafe runtime or online features slipped into v1
*** Add File: d:\rl-item-changer\bakkeswap\docs\PHASE_5_TAURI_GUI_STATUS.md
# Phase 5 Tauri GUI Status

## Goal

Phase 5 wires the first usable desktop GUI over the stable Rust backend contracts.

The frontend remains intentionally thin. It collects user intent, renders backend state, and calls Tauri commands that delegate to `bakkeswap-core` services.

## Implemented

- Tauri commands now cover the active desktop workflow:
	- status and config loading
	- game path validation and persistence
	- CodeRed import and database refresh
	- search
	- plan creation and saved-plan build
	- install preview and confirmation-gated install execution
	- installed-swap listing
	- restore preview and confirmation-gated restore execution
	- permanent originals backup status and verification
- the command bridge returns UI-facing DTOs instead of forcing the frontend to reconstruct backend state
- `AppStatus` now exposes indexed local file count for setup and overview screens
- search hits now expose product quality for the desktop search cards
- the desktop app is wired to the Tauri dialog plugin for local folder picking
- the Svelte app now provides the first usable screen set:
	- Home
	- Game Folder
	- Database
	- Quick Swap
	- Install Preview
	- Active Swaps
	- Backups
	- Logs
- the frontend store adds:
	- initial bootstrap loading
	- debounced TARGET and SOURCE search
	- typed confirmations for install and restore
	- loading states and runtime warnings
	- recent command logging
	- browser-only fallback when Tauri is unavailable

## Safety Boundary

Phase 5 does not introduce any new mutation path.

- the frontend does not rebuild packages or touch game files directly
- install and restore remain preview-first and confirmation-gated
- original-backup restore remains explicit emergency-only behavior
- GUI bring-up is still expected to use copied or fake `CookedPCConsole` roots first
- offline-only and local-only warnings remain prominent in the desktop shell

## Validation

The current Phase 5 validation gate passed after the GUI wiring landed:

- `npm run check`
- `cargo check -p bakkeswap-tauri`

The backend validation from the previous phases remains the foundation under the desktop layer.

## Remaining Gaps

- manual smoke validation of the full desktop flow against copied or fake Rocket League roots
- a dedicated settings screen
- packaging, signing, updater work, and release hardening
- broader golden-answer rebuild validation against real known-answer cases
*** Add File: d:\rl-item-changer\bakkeswap\docs\GUI_UX_FLOW.md
# GUI UX Flow

## Principles

- TARGET is the item the player already owns or equips
- SOURCE is the item the player wants to see locally
- the GUI is a control surface over Rust services, not a second implementation of swap logic
- preview results and confirmation phrases come from the backend and remain the source of truth

## Startup

1. On launch, the app loads status, config, installed swaps, and permanent-original backup status.
2. If Tauri is unavailable, the frontend enters browser-only mode and shows that desktop actions are disabled.
3. The Home page summarizes the current safety posture before the user starts planning or mutating files.

## Home

- show configured `CookedPCConsole` summary
- show indexed local file count, database counts, active swaps, and backup counts
- keep offline-only and sandbox-first rules visible
- allow a top-level refresh of backend state

## Game Folder

1. User pastes a path or picks a local folder.
2. Backend validation checks whether the path points at a Rocket League root, `TAGame`, or `CookedPCConsole`.
3. The GUI renders blockers and warnings returned by the backend.
4. Only after validation does the user persist the path.

## Database

1. User selects the folder containing `ProductDump.json` and related dump files.
2. Import runs through the backend importer.
3. Refresh updates the local file index and status counts.
4. Warnings and summaries stay visible so the user can tell whether the workspace is ready for search.

## Quick Swap

1. User searches TARGET products.
2. User searches SOURCE products.
3. Search is debounced and limited so the desktop shell stays responsive.
4. Only swappable product hits are selectable.
5. After both sides are selected, the user creates a plan and then builds it.

## Install Preview

1. The user requests install preview for the current built plan.
2. The backend returns blockers, warnings, file actions, and the exact confirmation phrase.
3. The GUI keeps the confirm action disabled until:
	 - preview status is ready
	 - blockers are empty
	 - the user types the exact backend-issued phrase
4. Optional overwrite of an existing profile backup remains an explicit checkbox, not a silent default.

## Active Swaps And Restore

1. The Active Swaps page lists installed profiles from local manifests and SQLite.
2. User selects one profile and requests restore preview.
3. The backend returns restore blockers, warnings, and the exact confirmation phrase.
4. Standard restore stays on the per-profile backup path.
5. Restore from permanent originals is an explicit emergency toggle and must remain visibly distinct.

## Backups

- show permanent-original backup tracking counts
- show missing-file counts clearly
- provide explicit verification on demand

## Logs

- show recent command start, success, and failure entries
- keep the log local to the current app session
- use it to explain backend activity without exposing a second command surface to the user