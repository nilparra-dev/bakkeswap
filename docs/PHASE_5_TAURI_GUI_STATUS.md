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