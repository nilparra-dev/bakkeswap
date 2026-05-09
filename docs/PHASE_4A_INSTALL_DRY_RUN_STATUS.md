# Phase 4A Install Dry-Run Status

## Goal

Phase 4A adds a real install dry-run preview from saved plans and saved sandbox builds.

This phase still does not copy, back up, restore, or modify any live Rocket League files.

## Implemented

- core installer preview service: `crates/bakkeswap-core/src/services/installer.rs`
- preview models:
  - `InstallPreview`
  - `InstallPreviewFile`
  - `BackupPreview`
  - `InstallWarning`
  - `InstallBlocker`
- preview behavior:
  - loads and validates saved `swap_plan.json`
  - requires `--dry-run` for all install preview flows in this phase
  - verifies the saved plan and last build report are in a successful build state
  - verifies built outputs still exist
  - verifies stored build-output hashes still match when validation data recorded them
  - resolves live install destinations under the currently configured `CookedPCConsole`
  - reports destination hash drift relative to the original target hash recorded in the plan
  - warns when the current configured cooked root differs from the plan cooked root
  - reports per-profile backup targets under `workspace/backups/<profile_name>/`
  - reports permanent original-backup targets under `workspace/original_files_backup/`
  - reports restore command and confirmation phrase placeholders without executing them
  - guarantees `dry_run_only = true` and `no_files_written = true`
- CLI command:
  - `bakkeswap install --plan <plan_path> --dry-run`
  - `bakkeswap install --plan <plan_path> --dry-run --json`
  - `bakkeswap install --plan <plan_path>` now refuses with `Real install is not implemented yet. Use --dry-run.`
- synthetic test coverage:
  - visual-only preview with no filesystem modification
  - visual plus thumbnail preview with exact backup paths
  - missing built output blocker
  - missing destination file blocker
  - destination hash drift warning
  - non-dry-run refusal

## Safety Boundary

The Phase 4A installer remains preview-only.

It may not:

- copy rebuilt files into `CookedPCConsole`
- create profile backups on disk
- create permanent original backups on disk
- write install manifests
- mark plans as installed
- insert install records into SQLite
- restore files
- modify real game files in place

## Validation

The following focused validation commands passed during the Phase 4A implementation:

- `cargo check -p bakkeswap-core`
- `cargo check -p bakkeswap-cli`
- `cargo test -p bakkeswap-core --test installer`

The full repo validation gate still applies before phase closeout:

- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`