# Building From Source

## Goal

This document describes the current developer setup path for the Rust core, CLI, and Phase 5 Tauri desktop GUI.

Building from source today is primarily for contributors working on the backend services, command bridge, and safe local desktop bring-up.

## Requirements

### Rust

- install the Rust stable toolchain
- ensure `cargo`, `rustc`, `rustfmt`, and `clippy` are available in your shell

### Node.js

- install Node.js
- install npm

### Windows Notes

On Windows, the MSVC toolchain also needs Windows SDK and Visual C++ build tools so the linker can find system libraries such as `kernel32.lib`.

Those libraries are required for the validated Rust and Tauri command set below.

## Validated Commands

From the repository root:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli -p bakkeswap-tauri`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
5. `npm install`
6. `npm run check`
7. `npm run build`

These commands are the current validation baseline for the Rust backend, CLI, and Svelte frontend.

## Desktop Dev Loop

From the repository root:

1. `npm run tauri:dev`
2. Use the Game Folder page to select a copied or fake Rocket League root, `TAGame`, or `CookedPCConsole`.
3. Use the Database page to import a local dump folder and refresh indexes.
4. Use Quick Swap to search TARGET and SOURCE, create a plan, build it, and request install preview.
5. Only exercise confirmed install and restore against copied or fake `CookedPCConsole` roots.

For frontend-only iteration, `npm run dev` is still useful, but the app will intentionally enter browser-only mode and display that the Rust backend is unavailable.

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

The equivalent GUI flow is:

1. `npm run tauri:dev`
2. Game Folder: set the fake root or `CookedPCConsole`
3. Database: import the fixture dump folder, then refresh the database
4. Quick Swap: search for the target and source products, create a plan, and build it
5. Install Preview: verify warnings and blockers before any confirmation-gated install
6. Active Swaps: use restore preview and confirmed restore only against the fake root

## Safety Rules While Developing

- do not commit `.upk` files
- do not commit real Rocket League assets
- do not commit inventory dumps
- do not test installs against the real `CookedPCConsole` during automation
- do not test the GUI install or restore flows against a live install during bring-up
- keep all validation sandboxed and offline/local only