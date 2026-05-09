# Phase 5B GUI Sandbox Smoke Status

## Goal

Phase 5B validates the desktop flow against a fully synthetic sandbox and hardens the UX around risky actions.

The goal is not packaging or overlay work. The goal is to prove that the current GUI can safely orchestrate the stable Rust backend against fake local data only.

## Implemented

- added a repeatable controlled smoke helper in `src-tauri/src/gui_sandbox_smoke.rs`
- the helper creates `target/gui_smoke` with:
  - fake CodeRed dumps
  - fake Rocket League root
  - fake `CookedPCConsole`
  - synthetic `.upk`-like files accepted by the rebuild path
  - a local app home and SQLite database
  - `smoke_report.json`
- the helper drives the Tauri command bridge through:
  - path validation and persistence
  - dump import
  - database refresh
  - TARGET and SOURCE search
  - plan creation
  - build
  - install preview and confirmed install
  - installed-swap listing
  - restore preview and confirmed restore
  - permanent originals backup status and verification
- the Svelte app was hardened with:
  - clearer TARGET and SOURCE language
  - explicit "item you own or equip" and "item you want to see" labels
  - path posture classification: sandbox or project-local, local custom, possible live install
  - clearer disabled states for save, import, and refresh actions
  - risk-context panels on install, restore, and backups screens
  - more obvious confirmation phrase presentation for install and restore
  - a Diagnostics screen for app home, database path, configured CookedPCConsole, counts, and backup roots
  - expected smoke-sequence guidance on the Logs page
- stale Game Folder validation state now clears when the input changes

## Smoke Result

The current passing controlled smoke run produced:

- fake path validation succeeded against `target/gui_smoke/RocketLeague`
- CodeRed import succeeded with 4 products
- refresh indexed 5 synthetic local package files
- TARGET and SOURCE search both found the expected products
- the plan profile `source_decal_on_target_decal` was created
- build completed with status `built`
- install preview completed with status `preview_ready`
- confirmed install completed with status `installed`
- the installed swap appeared as active
- restore preview completed with status `preview_ready`
- confirmed restore completed with status `restored`
- the installed swap record remained but became inactive
- permanent originals status and verification both reported `ready`

See `target/gui_smoke/smoke_report.json` for the exact captured values from the most recent controlled run.

## Safety Boundary

Phase 5B remains strictly local and offline.

- no real Rocket League install path is used in the controlled smoke flow
- no real `.upk` assets are committed
- no runtime hooks or overlays are introduced
- no anti-cheat bypass behavior is introduced
- the GUI still delegates all mutating operations to Rust services through Tauri commands

## Validation

Phase 5B adds one extra validation command on top of the existing checks:

- `cargo test -p bakkeswap-tauri gui_sandbox_smoke::controlled_gui_sandbox_smoke_flow -- --exact --nocapture --test-threads=1`

The full validation gate remains:

- `npm run check`
- `npm run build`
- `cargo check -p bakkeswap-tauri`
- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

## Remaining Gaps

- a human visual pass through the live GUI is still useful for final layout judgement and folder-picker ergonomics
- no packaging, signing, or updater work has been started
- no overlay or Player Title runtime work has been started