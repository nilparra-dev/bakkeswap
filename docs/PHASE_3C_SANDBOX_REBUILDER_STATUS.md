# Phase 3C Sandbox Rebuilder Status

## Goal

Phase 3C enables the first real Rust target-identity rebuild output, but only to an explicit sandbox path chosen by the operator.

This phase does not enable build, install, restore, or any write path into Rocket League `CookedPCConsole`.

## Implemented

- core API: `rebuild_target_identity(source_path, target_path, output_path, options)`
- sandbox safety checks for output paths:
  - output must end with `.upk`
  - output must not equal the source package path
  - output must not equal the target package path
  - output must not target `CookedPCConsole`
  - output must not target the configured cooked directory
  - parent directory must exist unless explicit directory creation is enabled
- sandbox rebuild validation after write:
  - output exists
  - output filename matches the target filename
  - output parses
  - output header tables decrypt
  - output body decompresses
  - output body hash matches the source body hash
  - target identity name is present
  - target export ref count covers the modified export refs
- CLI command: `bakkeswap upk rebuild-sandbox --source <path> --target <path> --output <path> [--create-dir] [--json]`
- known-answer integration: `bakkeswap upk known-answer --source <path> --target <path> [--expected <path>] [--output <path>] [--create-dir] [--json]`
- synthetic tests for sandbox rebuild success, sandbox path refusal, known-answer generation, and expected-output byte comparison

## Still Deferred

- plan-driven build execution from saved swap plans
- thumbnail rebuild output
- install and restore surfaces
- local known-answer sample validation against real developer-owned packages
- any write into the live game folder

## Validation

The following commands passed for this phase:

- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
