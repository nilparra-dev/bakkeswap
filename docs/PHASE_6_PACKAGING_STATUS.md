# Phase 6 Packaging Status

## Goal

Phase 6 creates the first local Windows beta package for BakkesSwap.

This phase is intentionally limited to local packaging and local validation. It does not introduce public release automation, signing, auto-updaters, overlays, runtime hooks, or any online behavior.

## Scope

Phase 6 covers:

- review of the current Tauri bundle config
- local Windows package build with `npm run tauri:build`
- assembly of a beta release folder under `dist/beta/`
- packaging documentation for local testers
- packaging smoke validation using only fake or sandbox data

Phase 6 does not cover:

- public distribution
- signed installers
- updater artifacts or updater infrastructure
- F2 overlay work
- Player Title runtime work
- real Rocket League paths in validation

## Packaging Review

The current Tauri desktop metadata is already sane for a beta candidate:

- product name: `BakkesSwap`
- main window title: `BakkesSwap`
- identifier: `com.bakkeswap.desktop`
- icon file present at `src-tauri/icons/icon.ico`
- no explicit extra bundle resources are configured

That means the bundle should package the app itself and frontend assets, not local sandbox outputs by default.

## Packaging Blocker Found And Fixed

The first Windows bundle attempt failed after producing the optimized release executable:

- Tauri built `target/release/bakkeswap-tauri.exe`
- Windows bundling then failed with `Couldn't find a .ico icon`

Root cause:

- the existing `src-tauri/icons/icon.ico` file was present
- the bundle config did not explicitly bind that `.ico` path for Windows bundling

Fix:

- added `bundle.icon = ["icons/icon.ico"]` to `src-tauri/tauri.conf.json`

After that change, `npm run tauri:build` succeeded.

## Current Outcome

Phase 6 now produces a first local Windows beta candidate with these bundle artifacts:

- `target/release/bundle/msi/BakkesSwap_0.1.0_x64_en-US.msi`
- `target/release/bundle/nsis/BakkesSwap_0.1.0_x64-setup.exe`

An optional release CLI artifact was also built successfully:

- `target/release/bakkeswap-cli.exe`

## Required Safety Checks

The beta package must not include:

- `target/gui_smoke`
- `local_samples`
- `local_output`
- committed or generated `.upk` files
- CodeRed dumps
- personal workspace state
- real Rocket League files

## Expected Outputs

Phase 6 should produce:

- Tauri Windows bundle artifacts from `npm run tauri:build`
- `dist/beta/BakkesSwap-v0.1.0-beta/`
- packaging docs and quick-start collateral for local testers
- optional SHA-256 checksums for packaged artifacts

## Validation Gate

The required validation gate for this phase is:

- `npm run check`
- `npm run build`
- `npm run tauri:build`
- `cargo fmt`
- `cargo check -p bakkeswap-tauri`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
- `cargo test -p bakkeswap-tauri gui_sandbox_smoke::controlled_gui_sandbox_smoke_flow -- --exact --nocapture --test-threads=1`

The bundle-specific checks completed as follows:

- `npm run build`: passed
- `npm run tauri:build`: passed after the explicit `.ico` bundle fix
- `cargo build -p bakkeswap-cli --release`: passed

## Packaged-App Smoke Result

The packaged app was smoke-validated on the sandbox boundary without touching real Rocket League paths.

Observed path:

- MSI artifact extracted to `target/beta_extract`
- packaged executable launched from `target/beta_extract/PFiles/BakkesSwap/bakkeswap-tauri.exe`
- `BAKKESWAP_APP_HOME` pointed at `target/gui_smoke/app_home`
- WebView2 remote debugging exposed a live packaged page titled `BakkesSwap Desktop`

Verified packaged window state:

- Home rendered successfully
- bridge state showed `TAURI READY`
- configured path showed the sandbox `CookedPCConsole`
- counts matched the smoke sandbox: `4 products`, `5 local .upk files`, `2 tracked backups`
- the packaged app rendered the same sandbox safety posture and warnings expected from Phase 5

This confirms the packaged executable can boot and render the validated sandbox workflow state using only fake local data.

## Release Folder

The assembled beta package should live at:

- `dist/beta/BakkesSwap-v0.1.0-beta/`

That folder should contain only safe release artifacts and documentation collateral.

## Packaging Smoke Goal

The packaged beta should be verified only against fake local data. The smoke pass must stay on the same sandbox safety boundary already proven by Phase 5B and Phase 5C.