# Building From Source

## Goal

This document describes the current developer setup path for the public Rust + Tauri rewrite.

The repository is still in skeleton-plus-foundation phase. Building from source today is primarily for contributors working on the Rust core, CLI, and Tauri shell.

## Requirements

### Rust

- install the Rust stable toolchain
- ensure `cargo`, `rustc`, `rustfmt`, and `clippy` are available in your shell

### Node.js

- install Node.js
- install npm

### Windows Notes

On Windows, the MSVC toolchain also needs Windows SDK and Visual C++ build tools so the linker can find system libraries such as `kernel32.lib`.

Without those libraries, `cargo check` will fail even if Rust itself is installed.

## CLI And Core Build Steps

From the repository root:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

## Frontend And Tauri Setup

From the repository root:

1. `npm install`
2. `npm run dev`
3. `npm run tauri:dev`

The GUI is not the main focus of Phase 1. The Rust database, config, import, indexing, and search foundation comes first.

## Safety Rules While Developing

- do not commit `.upk` files
- do not commit real Rocket League assets
- do not commit inventory dumps
- do not test installs against the real `CookedPCConsole` during automation
- keep all validation sandboxed and offline/local only