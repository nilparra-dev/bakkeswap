# Safety Model

## Scope

BakkesSwap v1 is local-only and offline-only.

The Rust rewrite is intentionally phased so each step proves a smaller safety boundary before any later step is allowed to mutate files.

The desktop GUI is an orchestration layer over those backend phases. It does not widen the file-mutation boundary.

## File Mutation Ladder

1. read-only package inspection
2. sandbox-only rebuild output
3. saved-plan sandbox build
4. install dry-run preview
5. backup preparation
6. confirmation-gated install execution
7. confirmation-gated restore execution

Each later step depends on the earlier step instead of bypassing it.

## GUI Command Boundary

- the Svelte frontend may browse local folders, capture text input, and render backend reports
- the frontend may not parse dumps, resolve swaps, rebuild packages, or write game files on its own
- every mutating desktop action must pass through a Tauri command that calls `bakkeswap-core`
- browser-only mode is intentionally non-mutating and must show that the backend is unavailable
- TARGET and SOURCE selection remains explicit in the GUI to reduce operator error

## Install Safety Rules

- install starts from a fresh preview of the saved plan and last build
- install blocks on preview issues
- install requires exact confirmation text `INSTALL <profile_name>`
- desktop install execution only enables after the preview returns that exact confirmation phrase
- install prepares permanent original backups before any file replacement
- install prepares per-profile backups before any file replacement
- install verifies installed hashes after copy
- install writes `install_manifest.json` so restore can consume an exact local record later

## Restore Safety Rules

- normal restore uses the per-profile backup manifest first
- restore verifies backup file existence and backup hashes before copy
- restore requires exact confirmation text `RESTORE <profile_name>`
- desktop restore execution only enables after restore preview returns that exact confirmation phrase
- restore blocks any destination path that escapes the configured `CookedPCConsole`
- restore verifies restored destination hashes after copy
- restore updates `restored_at` metadata in local manifests and SQLite when available

## Emergency Originals Restore

- permanent originals are not the default restore path
- originals restore requires `--from-originals`
- originals restore requires exact confirmation text `RESTORE ORIGINALS <profile_name>`
- the desktop GUI must present originals restore as an explicit emergency path, never the default action
- originals restore emits an explicit warning because it is an emergency path
- originals restore exists only for local recovery when the per-profile backup is missing or invalid

## Setup And Validation Rules

- GUI bring-up must use copied or fake `CookedPCConsole` roots first
- desktop folder pickers only target local directories supplied by the user
- automated validation may compile the Tauri app, but it may not point at a live Rocket League install
- backend preview results remain the source of truth for blockers, warnings, and confirmation phrases

## Hard Boundaries

- no real Rocket League install paths in automated tests
- no committed `.upk` assets
- no runtime hooks
- no EAC bypass
- no online features or remote services
- no frontend-side file mutation path that bypasses the Rust services
- no hidden fallback that widens file-mutation scope automatically
