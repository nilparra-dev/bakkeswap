# Building From Source

## Goal

This document describes the current developer setup path for the public Rust + Tauri rewrite.

Phase 1 database/search work is complete and the Phase 2 planner foundation is now implemented.

Building from source today is primarily for contributors working on the Rust core, CLI, planner flow, and the Tauri shell.

## Requirements

### Rust

- install the Rust stable toolchain
- ensure `cargo`, `rustc`, `rustfmt`, and `clippy` are available in your shell

### Node.js

- install Node.js
- install npm

### Windows Notes

On Windows, the MSVC toolchain also needs Windows SDK and Visual C++ build tools so the linker can find system libraries such as `kernel32.lib`.

Those libraries are required for the validated Rust command set below.

## CLI And Core Build Steps

From the repository root:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

These commands were run successfully on the current Windows machine after Rust and Visual Studio Build Tools were installed.

## Frontend And Tauri Setup

From the repository root:

1. `npm install`
2. `npm run dev`
3. `npm run tauri:dev`

The GUI is still not the main implementation focus. The Rust database, config, import, indexing, search, and planner foundations come first.

## Fake Fixture Smoke Workflow

The repository includes safe fake metadata fixtures under:

- `crates/bakkeswap-core/tests/fixtures/codered_minimal`
- `crates/bakkeswap-core/tests/fixtures/codered_planner`

For manual CLI smoke tests, create a temporary app home and a fake Rocket League install root with a fake `TAGame/CookedPCConsole` directory. Then create empty or fake-content `.upk` files whose names match the planner fixture packages:

- `Skin_Target_SF.upk`
- `Skin_Target_T_SF.upk`
- `Skin_Source_SF.upk`
- `Skin_Source_T_SF.upk`

Example smoke sequence from the repository root:

1. set `BAKKESWAP_APP_HOME` to a temporary folder
2. `cargo run -p bakkeswap-cli -- db import-codered crates/bakkeswap-core/tests/fixtures/codered_planner`
3. `cargo run -p bakkeswap-cli -- config set-game-path <fake_rocket_league_root>`
4. `cargo run -p bakkeswap-cli -- db refresh`
5. `cargo run -p bakkeswap-cli -- search "Target Decal"`
6. `cargo run -p bakkeswap-cli -- plan --target 1001 --source 1002`

## Safety Rules While Developing

- do not commit `.upk` files
- do not commit real Rocket League assets
- do not commit inventory dumps
- do not test installs against the real `CookedPCConsole` during automation
- keep all validation sandboxed and offline/local only