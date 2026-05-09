# Safety Model

## Scope

BakkesSwap v1 is local-only and offline-only.

The Rust rewrite is intentionally phased so each step proves a smaller safety boundary before any later step is allowed to mutate files.

## File Mutation Ladder

1. read-only package inspection
2. sandbox-only rebuild output
3. saved-plan sandbox build
4. install dry-run preview
5. backup preparation
6. confirmation-gated install execution
7. confirmation-gated restore execution

Each later step depends on the earlier step instead of bypassing it.

## Install Safety Rules

- install starts from a fresh preview of the saved plan and last build
- install blocks on preview issues
- install requires exact confirmation text `INSTALL <profile_name>`
- install prepares permanent original backups before any file replacement
- install prepares per-profile backups before any file replacement
- install verifies installed hashes after copy
- install writes `install_manifest.json` so restore can consume an exact local record later

## Restore Safety Rules

- normal restore uses the per-profile backup manifest first
- restore verifies backup file existence and backup hashes before copy
- restore requires exact confirmation text `RESTORE <profile_name>`
- restore blocks any destination path that escapes the configured `CookedPCConsole`
- restore verifies restored destination hashes after copy
- restore updates `restored_at` metadata in local manifests and SQLite when available

## Emergency Originals Restore

- permanent originals are not the default restore path
- originals restore requires `--from-originals`
- originals restore requires exact confirmation text `RESTORE ORIGINALS <profile_name>`
- originals restore emits an explicit warning because it is an emergency path
- originals restore exists only for local recovery when the per-profile backup is missing or invalid

## Hard Boundaries

- no real Rocket League install paths in automated tests
- no committed `.upk` assets
- no runtime hooks
- no EAC bypass
- no online features or remote services
- no hidden fallback that widens file-mutation scope automatically
