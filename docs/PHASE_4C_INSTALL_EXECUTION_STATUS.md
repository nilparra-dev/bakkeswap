# Phase 4C Install Execution Status

## Goal

Phase 4C adds confirmation-gated real install execution on top of the Phase 4A preview and Phase 4B backup managers.

This phase allows local file replacement only after a fresh preview, explicit confirmation, and mandatory backup preparation.

## Implemented

- core install execution service: `crates/bakkeswap-core/src/services/installer.rs`
- new install models:
  - `InstalledFileRecord`
  - `InstallReport`
  - `SwapPlan.install_status`
  - `SwapPlan.last_install`
- install execution behavior:
  - always starts from a fresh preview of the saved plan and last build report
  - blocks when preview blockers exist
  - requires exact confirmation text `INSTALL <profile_name>`
  - prepares permanent original backups before any file replacement
  - prepares per-profile backups before any file replacement
  - refuses existing profile backup reuse unless overwrite is explicitly requested
  - copies validated built outputs into the configured `CookedPCConsole`
  - verifies installed hashes against the built outputs after copy
  - writes `workspace/backups/<profile_name>/install_manifest.json`
  - updates the saved plan JSON with final install status and report data
  - writes `installed_swaps` metadata when the plan already exists in SQLite
- CLI install behavior:
  - `bakkeswap install --plan <plan_path> --dry-run`
  - `bakkeswap install --plan <plan_path> --confirm "INSTALL <profile_name>"`
  - `bakkeswap install --plan <plan_path> --overwrite-profile-backup`
  - interactive confirmation fallback in human-readable mode when `--confirm` is omitted
- synthetic sandbox test coverage:
  - missing confirmation refusal
  - wrong confirmation refusal
  - preview blocker refusal
  - visual-only install success
  - visual plus thumbnail install success
  - profile backup reuse refusal without overwrite
  - overwrite-enabled reinstall success
  - install manifest and plan JSON updates
  - SQLite `installed_swaps` persistence when plan metadata exists
  - path-escape blocking for unsafe target filenames

## Safety Boundary

Phase 4C remains local-only and sandbox-first.

It may not:

- restore files yet
- skip the install preview or backup preflight
- target the real Rocket League install during automated validation
- introduce runtime hooks, EAC bypass behavior, or online features

## Validation

The following validation commands passed during the Phase 4C implementation:

- `cargo fmt`
- `cargo check -p bakkeswap-core`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core --test installer`
- `cargo test -p bakkeswap-core`
- `cargo check -p bakkeswap-cli`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

Still deferred from full feature completion:

- manual real-install operator checklist
- restore implementation and validation
