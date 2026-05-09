# Rust CLI Command Design

## Goal

The Rust CLI is the first implementation milestone and the authoritative backend contract for the Tauri GUI.

It must preserve the proven Python command behavior and safety model before the GUI becomes more than a shell.

## Command Set

### `bakkeswap selftest`

Purpose:

- verify local runtime directories, schema availability, and known-sample readiness

Required future checks:

- SQLite open and migrations applied
- configured game path state
- sandbox install path availability for tests
- sample package availability for golden validation

### `bakkeswap config set-game-path <path>`

Purpose:

- save the real Rocket League game path after normalizing to CookedPCConsole

Required future behavior:

- validate path
- store normalized cooked root
- warn if database refresh is required because the active install path changed

### `bakkeswap config show`

Purpose:

- print current configuration and detected game-path candidates

### `bakkeswap config validate`

Purpose:

- validate current configured game path and return a non-zero exit code on failure

### `bakkeswap db import-codered <folder>`

Purpose:

- import raw CodeRed dump files into SQLite

Required future behavior:

- import products, slots, paints, titles
- preserve counts for auditing
- support the known 8955-product baseline when current dumps match the validated source set

### `bakkeswap db refresh`

Purpose:

- refresh imported metadata and local file availability state

Required future behavior:

- re-run import and local file indexing without requiring owned-items JSON

### `bakkeswap search <query>`

Purpose:

- query products by name, ID, slot, or local package identity

Required future behavior:

- fast local search from SQLite
- clear TARGET versus SOURCE selection semantics in future UI integration

### `bakkeswap upk inspect <path>`

Purpose:

- inspect a local `.upk` file in read-only mode

Current implementation:

- reads package magic, file version, and licensee version
- parses the package summary/header fields used by the Python prototype
- detects the Rocket League UE3 package profile heuristically
- decrypts the header table region with the solved Rocket League AES-256-ECB key
- parses NameTable, ImportTable, ExportTable, and DependsTable when the decrypted region is valid
- parses Rocket League compressed chunk metadata and decompresses the body with zlib
- computes file SHA-256 and decompressed body SHA-256
- supports human-readable output by default and `--json` output for machine-readable inspection

Still deferred:

- any package writing
- thumbnail rebuild logic
- plan-driven build/install integration
- install or restore operations

### `bakkeswap upk known-answer --source <path> --target <path> [--expected <path>] [--output <path>] [--create-dir]`

Purpose:

- compare Rust sandbox rebuild behavior against a known-answer package without enabling install flows

Current implementation:

- inspects source and target packages in read-only mode
- inspects the expected known-answer package when provided
- optionally generates a sandbox-only rebuilt output when `--output` is provided
- validates that the generated output parses, decrypts, decompresses, preserves the source body, and exposes the target identity
- compares generated output bytes against the expected package when both are provided
- supports human-readable output by default and `--json` for machine-readable reports

Still deferred:

- any write path that targets the live game install
- any plan/build/install/restore orchestration around known-answer runs

### `bakkeswap upk rebuild-sandbox --source <path> --target <path> --output <path> [--create-dir]`

Purpose:

- emit a rebuilt target-identity `.upk` to an explicit sandbox path only

Current implementation:

- parses source and target packages
- preserves the source decompressed body
- ensures the target identity exists in the NameTable
- rewrites matching export object-name refs to the target identity
- shifts nonzero export serial offsets by the header name-table delta
- re-encrypts the header table region and re-emits chunk metadata
- validates the written output by reparsing, decrypting, decompressing, and comparing body hashes
- rejects output paths that target the source file, target file, `CookedPCConsole`, or the configured cooked directory

Still deferred:

- plan-driven build execution
- thumbnail rebuild output
- any install or restore flow

### `bakkeswap plan --target <product_id> --source <product_id>`

Purpose:

- resolve a swap plan using the target-identity rebuild path

Current implementation:

- loads target and source products from SQLite
- verifies both products exist
- verifies slot compatibility
- blocks non-swappable product types such as Player Title products
- resolves visual and thumbnail package filenames from imported metadata plus `local_files`
- writes `workspace/plans/<profile_name>/swap_plan.json` under the app home
- stores plan metadata in the `swap_plans` SQLite table
- supports human-readable output by default and `--json` for machine-readable output

Still deferred:

- build/rebuild execution
- install execution
- restore execution
- any direct game-file modification from the plan command

### `bakkeswap build --plan <plan_path>`

Purpose:

- execute sandbox-only rebuild output from a saved `swap_plan.json`

Current implementation:

- loads the saved plan JSON from disk
- verifies the supported `schema_version`
- refuses plans with recorded build blockers
- warns when the current configured cooked root differs from the cooked root recorded in the plan
- rebuilds the visual operation through the Phase 3C sandbox rebuilder
- rebuilds the thumbnail operation when the plan resolved one and skips it otherwise
- writes outputs under `workspace/builds/<profile_name>/` by default
- supports `--output-root <folder>` to redirect outputs to another explicit sandbox root
- updates the plan JSON with build status and validation results
- writes a build record into SQLite when the plan already exists in the `swap_plans` table
- supports human-readable output by default and `--json` for machine-readable reports

Still deferred:

- any output path that targets the live game install
- install orchestration
- restore orchestration
- plan-driven backup handling
- real install execution from saved builds

### `bakkeswap install --plan <plan_path> --dry-run`

Purpose:

- preview exact install actions without touching the real game files

Current implementation:

- loads the saved `swap_plan.json`
- verifies the saved plan and last build report are in a successful build state
- verifies the current configured `CookedPCConsole` exists and resolves install destinations under it
- verifies built outputs still exist and still match stored build validation hashes when available
- reports every `CookedPCConsole` file that would be overwritten
- reports every rebuilt file that would be installed
- reports per-profile backup targets under `workspace/backups/<profile_name>/`
- reports permanent original-backup targets under `workspace/original_files_backup/`
- warns when the configured cooked root differs from the plan's cooked root
- warns when the current destination hash differs from the original target hash recorded in the plan
- supports human-readable output by default and `--json` for machine-readable preview data

Still deferred:

- copying any files into `CookedPCConsole`
- creating profile backups on disk
- creating permanent original backups on disk
- restore execution

### `bakkeswap install --plan <plan_path> [--confirm "INSTALL <profile_name>"] [--overwrite-profile-backup]`

Purpose:

- perform a real local install only after explicit operator confirmation

Current implementation:

- starts from a fresh install preview instead of trusting stale install state
- blocks when the preview is not `preview_ready`
- requires an exact confirmation phrase of the form `INSTALL <profile_name>`
- supports `--confirm "INSTALL <profile_name>"` for non-interactive execution
- prompts interactively for the confirmation phrase when `--confirm` is omitted in human-readable mode
- prepares permanent original backups before any file replacement
- prepares per-profile backups before any file replacement
- refuses existing profile backup reuse unless `--overwrite-profile-backup` is set
- copies validated built outputs into the configured cooked root only after the backup preflight passes
- verifies installed destination hashes against the built outputs after copy
- writes `workspace/backups/<profile_name>/install_manifest.json`
- updates the saved plan JSON with `install_status` and `last_install`
- writes `installed_swaps` metadata when the plan already exists in SQLite

Still deferred:

- restore execution
- any automated test or workflow that targets a real Rocket League install path
- any install path that bypasses preview or backup preflight

### `bakkeswap restore --profile <profile_name> --dry-run`

Purpose:

- preview exact restore actions without touching the cooked root

Current implementation:

- loads `workspace/backups/<profile_name>/manifest.json` by default
- verifies backup files exist and verifies backup hashes before any real restore path can proceed
- resolves restore destinations under the cooked root recorded in `install_manifest.json` when available
- falls back to the currently configured cooked root when `install_manifest.json` is unavailable
- blocks any restore path whose relative file path would escape the configured `CookedPCConsole`
- reports exact backup source files, destination files, and hash state
- writes nothing during dry-run preview
- supports `--from-originals` to preview the emergency permanent-original restore path explicitly

### `bakkeswap restore --profile <profile_name> --confirm "RESTORE <profile_name>"`

Purpose:

- restore files from the per-profile backup set after explicit confirmation

Current implementation:

- requires exact confirmation text `RESTORE <profile_name>`
- refuses plain restore without the correct confirmation phrase
- restores from `workspace/backups/<profile_name>/manifest.json` by default
- verifies backup file hashes before copying any file back into `CookedPCConsole`
- verifies restored destination hashes after copy
- updates `install_manifest.json` with `restored_at` when the install manifest exists
- updates the saved plan JSON with `install_status = restored` and `last_install.restored_at` when the plan is available
- updates the active `installed_swaps` row with `restored_at` and `active = 0` when the plan exists in SQLite
- supports human-readable output by default and `--json` for machine-readable reports

### `bakkeswap restore --profile <profile_name> --from-originals --confirm "RESTORE ORIGINALS <profile_name>"`

Purpose:

- run the explicit emergency fallback restore path from permanent original backups

Current implementation:

- is never used automatically as a fallback from the normal profile-restore path
- requires both `--from-originals` and exact confirmation text `RESTORE ORIGINALS <profile_name>`
- restores from `workspace/original_files_backup/manifest.json`
- emits an explicit emergency-path warning in the restore report
- still enforces path safety and hash verification before any file copy

Still deferred:

- GUI wiring for restore workflows
- manual operator checklist for non-sandbox restore use

### `bakkeswap status`

Purpose:

- print current configuration, database, install, and backup readiness

### `bakkeswap backup originals status`

Purpose:

- show permanent original-backup coverage and verification state

Current implementation:

- inspects `workspace/original_files_backup/manifest.json`
- reports tracked, missing, mismatched, and untracked backup files
- does not write or repair any backup data

### `bakkeswap backup originals verify`

Purpose:

- verify permanent original-backup hashes and manifest integrity

Current implementation:

- reads the permanent original backup manifest
- hashes tracked backup files under `workspace/original_files_backup/`
- reports missing files and hash mismatches as blockers
- refuses silently repairing or overwriting original backups

### `bakkeswap backup originals prepare --plan <plan_path>`

Purpose:

- create permanent original backups from the current destination files named by an install preview

Current implementation:

- loads the saved plan through the existing install dry-run preview
- reads the current destination files in the configured `CookedPCConsole`
- copies each untouched destination file once into `workspace/original_files_backup/`
- writes or updates `workspace/original_files_backup/manifest.json`
- verifies copied hashes after every backup write
- refuses to overwrite existing permanent originals automatically
- supports human-readable output by default and `--json` for machine-readable reports

Still deferred:

- any automatic refresh or overwrite flow for permanent originals
- repair commands for inconsistent original backup state

### `bakkeswap backup profile prepare --plan <plan_path>`

Purpose:

- create the per-profile backup set that a later restore flow will consume

Current implementation:

- loads the saved plan through the existing install dry-run preview
- copies current destination files into `workspace/backups/<profile_name>/`
- writes `workspace/backups/<profile_name>/manifest.json`
- verifies copied hashes after every backup write
- refuses to reuse an existing profile backup folder by default
- supports `--overwrite-profile-backup` for explicit replacement
- supports human-readable output by default and `--json` for machine-readable reports

Still deferred:

- restore execution from the profile manifest
- automatic profile backup creation as part of real install

## Output Design Rules

- CLI output should be structured and machine-readable where possible
- dry-run output must stay human-readable enough for manual verification
- error messages must say why the operation is blocked and what the operator should fix next
- commands that would touch real game files must remain explicit and confirmation-gated
- plan output should preserve blockers and warnings instead of silently guessing around missing packages
- UPK inspection output must remain read-only and should prefer warnings over silent partial parses when non-fatal inspection steps fail

## Exit Code Rules

- `0`: success
- `1`: operator or validation failure
- `2`: configuration invalid or missing
- `3`: unsupported plan or build contract failure
- `4`: install safety check failure

## Safety Rules

- automated validation must use sandbox paths only
- sandbox rebuild commands must require an explicit output path outside `CookedPCConsole`
- saved-plan builds must write only to `workspace/builds` or another explicit sandbox output root
- no real CookedPCConsole writes during tests
- no online or anti-cheat-adjacent behavior
- no server inventory changes
- no runtime memory editing or injected overlay work in v1