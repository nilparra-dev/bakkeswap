# Phase 2 Planner Status

Date: 2026-05-09

## Goal

Phase 2 adds real swap planning in Rust without implementing the UPK rebuilder, build execution, install execution, restore execution, runtime hooks, or any direct game-file modification.

Status: complete and validated for the planner foundation scope.

## Implemented In This Pass

### Planner service

- `PlannerService` now loads target and source products from SQLite
- verifies both products exist
- checks slot compatibility
- blocks non-swappable package types such as Player Title products
- resolves package filenames from imported metadata plus indexed `local_files`
- derives visual and thumbnail identities from resolved package filenames
- collects warnings and build blockers instead of guessing around missing data

### Plan output

- saves JSON plans under `workspace/plans/<profile_name>/swap_plan.json` inside the app home
- stores plan metadata in the `swap_plans` SQLite table
- includes:
  - schema version
  - created timestamp
  - profile name
  - offline-only flag
  - database path
  - configured cooked root
  - target product details
  - source product details
  - compatibility checks
  - visual and thumbnail operations
  - warnings
  - build blockers
  - rollback-note placeholders

### CLI

- `bakkeswap plan --target <id> --source <id>` now runs the real planner
- default output is human-readable
- `--json` prints the saved plan payload in machine-readable form

### Metadata improvements used by the planner

- CodeRed import now derives candidate `visual_upk` and `thumb_upk` names from package metadata
- local file indexing normalizes Windows cooked-root paths consistently with config path storage

## Validated Test Coverage

The planner foundation is covered by safe fake-fixture tests for:

1. successful same-slot planning
2. missing target product
3. missing source product
4. slot mismatch
5. missing target visual package
6. missing source visual package
7. missing thumbnail packages warning without blocking a valid visual plan
8. Player Title product blocked as non-swappable

All planner tests use fake metadata plus temporary runtime-created `.upk` files only.

## Validation Result

The full Rust validation set passed after the planner implementation:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

## Intentionally Still Deferred

- UPK parser/rebuilder port
- `build` command implementation beyond stubs
- `install` command implementation beyond stubs
- `restore` command implementation beyond stubs
- any modification of real `CookedPCConsole` content
- runtime memory editing or Player Title runtime modules

## Safety State

- no real Rocket League files were used in tests
- no `.upk` assets were committed as fixtures
- no personal absolute paths were added to docs or fixtures
- offline/local-only boundaries remain intact