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
- target-identity rebuild logic
- table re-encryption for rebuilds
- offset rewriting
- install or restore operations

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

- create rebuilt output files for a saved plan

Required future behavior:

- reuse the proven target-identity rebuild method
- validate body equality and export-reference changes
- save build record and validation results

### `bakkeswap install --plan <plan_path> --dry-run`

Purpose:

- preview exact install actions without touching the real game files

Required future behavior:

- show exact target files
- show exact rebuilt files
- show exact backup targets
- show permanent original-backup state
- show restore command or restore target

### `bakkeswap install --plan <plan_path>`

Purpose:

- perform a real local install only after explicit operator confirmation

Required future behavior:

- verify permanent original backups
- create profile backups
- copy validated built outputs into the configured cooked root
- record install manifest

### `bakkeswap restore --profile <profile_name>`

Purpose:

- restore files from profile backup state

Required future behavior:

- restore exact files for the named profile
- clear active install state after a successful restore

### `bakkeswap status`

Purpose:

- print current configuration, database, install, and backup readiness

### `bakkeswap backup originals status`

Purpose:

- show permanent original-backup coverage and verification state

### `bakkeswap backup originals verify`

Purpose:

- verify permanent original-backup hashes and manifest integrity

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
- no real CookedPCConsole writes during tests
- no online or anti-cheat-adjacent behavior
- no server inventory changes
- no runtime memory editing or injected overlay work in v1