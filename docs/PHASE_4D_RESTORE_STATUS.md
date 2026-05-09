# Phase 4D Restore Status

## Goal

Phase 4D adds sandbox-first restore behavior on top of the Phase 4B backup managers and the Phase 4C install manifest.

Restore is confirmation-gated, verifies backup hashes before copy, verifies restored hashes after copy, and only uses permanent originals through an explicit emergency path.

## Implemented

- core restore service: `crates/bakkeswap-core/src/services/restore.rs`
- new restore request types:
  - `RestorePreviewRequest`
  - `RestoreExecutionRequest`
- new restore models:
  - `RestoreReport`
  - `RestoreFileRecord`
  - `RestoreWarning`
  - `RestoreBlocker`
- install manifest upgrade:
  - `InstallReport.restored_at`
- normal restore behavior:
  - dry-run preview reads `workspace/backups/<profile_name>/manifest.json`
  - verifies backup file existence and backup hashes before any copy
  - resolves destinations under the cooked root recorded in `install_manifest.json` when present
  - falls back to the currently configured cooked root when `install_manifest.json` is unavailable
  - blocks any restore path that escapes the configured `CookedPCConsole`
  - copies verified profile backup files back into the cooked root only after exact confirmation
  - verifies restored destination hashes after copy
  - updates `install_manifest.json` with `restored_at` when available
  - updates saved plan JSON to `install_status = restored` and stamps `last_install.restored_at` when the plan is available
  - updates `installed_swaps.restored_at` and clears `active` when the plan exists in SQLite
- emergency fallback behavior:
  - requires `--from-originals`
  - requires exact confirmation text `RESTORE ORIGINALS <profile_name>`
  - restores from `workspace/original_files_backup/manifest.json`
  - emits an explicit emergency-path warning
  - is never used automatically by the normal profile-restore path
- CLI restore behavior:
  - `bakkeswap restore --profile <profile_name> --dry-run`
  - `bakkeswap restore --profile <profile_name> --confirm "RESTORE <profile_name>"`
  - `bakkeswap restore --profile <profile_name> --from-originals --confirm "RESTORE ORIGINALS <profile_name>"`
  - `--json` support for preview and execution
- synthetic sandbox test coverage:
  - successful restore dry-run
  - successful restore from profile backup
  - wrong confirmation refusal
  - missing confirmation refusal
  - missing profile backup blocker
  - tampered profile backup hash blocker
  - path escape blocker
  - restored destination hash verification
  - install manifest `restored_at` update
  - saved plan install-status update
  - SQLite `installed_swaps` update
  - explicit originals fallback refusal without flag and correct confirmation
  - successful emergency originals fallback in sandbox

## Safety Boundary

Phase 4D remains offline, local-only, and sandbox-first.

It may not:

- touch a real Rocket League install during automated validation
- skip backup hash verification or restored hash verification
- fall back to permanent originals implicitly
- introduce runtime hooks, EAC bypass behavior, or online features

## Validation

The full requested validation gate passed during Phase 4D implementation:

- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core --test restore`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
