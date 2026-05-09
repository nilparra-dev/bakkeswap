# Phase 4B Backup Managers Status

## Goal

Phase 4B adds real backup-manager behavior behind the paths that Phase 4A already previewed.

This phase still does not install rebuilt outputs, restore files, or modify live Rocket League files in place.

## Implemented

- core backup service: `crates/bakkeswap-core/src/services/backups.rs`
- typed backup models:
  - `OriginalBackupManifest`
  - `OriginalBackupEntry`
  - `ProfileBackupManifest`
  - `ProfileBackupEntry`
  - `BackupResult`
  - `BackupVerificationResult`
  - `BackupWarning`
  - `BackupBlocker`
- permanent original backup behavior:
  - accepts a validated `InstallPreview`
  - copies current destination files into `workspace/original_files_backup/`
  - writes and verifies `workspace/original_files_backup/manifest.json`
  - reuses existing permanent originals when the manifest and file hashes still match
  - blocks on missing tracked files, untracked existing files, or hash mismatches
  - never overwrites permanent originals automatically
- per-profile backup behavior:
  - accepts a validated `InstallPreview`
  - copies current destination files into `workspace/backups/<profile_name>/`
  - writes and verifies `workspace/backups/<profile_name>/manifest.json`
  - refuses to reuse an existing profile backup by default
  - supports explicit replacement through `--overwrite-profile-backup`
  - verifies the existing profile backup before allowing overwrite
- CLI commands:
  - `bakkeswap backup originals status`
  - `bakkeswap backup originals verify`
  - `bakkeswap backup originals prepare --plan <plan_path> [--json]`
  - `bakkeswap backup profile prepare --plan <plan_path> [--overwrite-profile-backup] [--json]`
- synthetic test coverage:
  - permanent original backup creation
  - permanent original backup non-overwrite second run
  - permanent original backup verification
  - permanent original backup hash mismatch blocker
  - profile backup creation
  - profile backup reuse refusal by default
  - profile backup overwrite verification blocker
  - no install and no destination mutation during backup preparation

## Safety Boundary

The Phase 4B backup managers remain sandbox-safe.

They may not:

- copy rebuilt outputs into `CookedPCConsole`
- modify destination game files in place
- perform restore
- mark plans as installed
- write install manifests or install records
- overwrite permanent original backups automatically

## Validation

The following focused validation commands passed during the Phase 4B implementation:

- `cargo check -p bakkeswap-core`
- `cargo test -p bakkeswap-core --test backups`
- `cargo check -p bakkeswap-cli`

The full repo validation gate still applies before phase closeout:

- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`