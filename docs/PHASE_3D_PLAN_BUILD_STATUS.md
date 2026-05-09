# Phase 3D Plan Build Status

## Goal

Phase 3D wires saved `swap_plan.json` files to sandbox-only build execution.

This phase reuses the Phase 3C target-identity rebuilder, but it still does not implement install, restore, or any write into Rocket League `CookedPCConsole`.

## Implemented

- core build service: `crates/bakkeswap-core/src/services/builder.rs`
- request surface:
  - plan path
  - optional output root
  - `create_dir` flag
- build behavior:
  - loads and validates saved plan JSON
  - verifies supported `schema_version`
  - refuses plans with build blockers
  - warns if the current configured cooked root differs from the cooked root recorded in the plan
  - rebuilds the visual operation through `rebuild_target_identity`
  - rebuilds the thumbnail operation when the plan resolved one
  - skips thumbnail output when the saved plan is visual-only
  - writes outputs under `workspace/builds/<profile_name>/` by default
  - supports explicit sandbox output roots through `--output-root`
  - updates the saved plan JSON with build status and validation results
  - persists build metadata to SQLite when the plan already exists in `swap_plans`
- CLI command:
  - `bakkeswap build --plan <plan_path>`
  - `bakkeswap build --plan <plan_path> --output-root <folder>`
  - `bakkeswap build --plan <plan_path> --create-dir`
  - `bakkeswap build --plan <plan_path> --json`
- synthetic test coverage:
  - visual-only saved-plan build
  - visual plus thumbnail saved-plan build
  - blocked plan refusal
  - CookedPCConsole output-root rejection
  - missing source-path blocker
  - missing target-path blocker
  - plan JSON update after build

## Safety Boundary

The Phase 3D builder remains sandbox-only.

It may not:

- write into `CookedPCConsole`
- install files into the live game folder
- restore files
- modify game files in place
- commit `.upk` outputs
- perform any runtime hook or anti-cheat-adjacent action

## Validation

The following commands passed after the Phase 3D implementation:

- `cargo check -p bakkeswap-core`
- `cargo test -p bakkeswap-core`
- `cargo check -p bakkeswap-cli`

The full repo validation gate is still required before phase closeout:

- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
