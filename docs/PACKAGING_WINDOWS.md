# Packaging Windows

## Goal

This document describes the current Windows beta packaging path for BakkesSwap.

The result is a local beta candidate for testing, not a public stable release.

## Preconditions

- Rust stable toolchain installed
- Node.js and npm installed
- Tauri Windows prerequisites installed
- repository validations passing

## Build Commands

From the repository root:

1. `npm run check`
2. `npm run build`
3. `npm run tauri:build`

If the bundle succeeds, Tauri will place Windows artifacts under `src-tauri/target/release/bundle/`.

In this repository's current workspace build, the concrete outputs are:

- `target/release/bundle/msi/BakkesSwap_0.1.0_x64_en-US.msi`
- `target/release/bundle/nsis/BakkesSwap_0.1.0_x64-setup.exe`

The optimized desktop executable is also produced at:

- `target/release/bakkeswap-tauri.exe`

## Bundle Hygiene Rules

Before shipping a beta folder, confirm the bundle configuration does not package or copy:

- fake smoke roots from `target/gui_smoke`
- local build outputs from developer workspaces
- `.upk` files from experiments or backups
- CodeRed dump folders
- personal machine paths

The beta package should contain only the compiled app artifacts plus safe documentation collateral.

## Windows Icon Requirement

The Windows bundler requires an explicit `.ico` bundle icon path in this workspace.

The validated configuration uses:

- `src-tauri/icons/icon.ico`
- `bundle.icon = ["icons/icon.ico"]`

## Beta Folder Assembly

Create this folder:

- `dist/beta/BakkesSwap-v0.1.0-beta/`

Copy in:

- the Windows artifacts produced by Tauri
- `README.md`
- `OFFLINE_ONLY_WARNING.md`
- `docs/SAFETY_MODEL.md`
- `docs/GUI_UX_FLOW.md`
- `docs/BUILDING_FROM_SOURCE.md`
- `docs/QUICK_START_BETA.md` as `QUICK_START.md`

Optional additions:

- CLI executable if it is built cleanly and is useful for testers
- `SHA256SUMS.txt`

## Local Package Smoke Rules

The packaged app must only be tested against fake local data.

Allowed:

- `target/gui_smoke/app_home`
- `target/gui_smoke/codered_dumps`
- `target/gui_smoke/RocketLeague`

Not allowed:

- real Rocket League install roots
- real `.upk` assets
- personal inventory dumps

## Automation Note

For automated local smoke work, the MSI artifact can be extracted administratively to a local temp folder, for example `target/beta_extract`, and the extracted packaged executable can be launched from there.

That keeps the smoke test on packaged app files without requiring a manual installer flow.

## Manual Smoke Sequence

1. Start the packaged app.
2. Confirm Home loads.
3. Open Game Folder and validate `target/gui_smoke/RocketLeague`.
4. Open Database and import `target/gui_smoke/codered_dumps`.
5. Refresh DB.
6. Open Quick Swap and search TARGET `Target Decal` and SOURCE `Source Decal`.
7. Create plan.
8. Build plan.
9. Open Install Preview.
10. Confirm the configured path is still the sandbox path.
11. Confirm install only against the sandbox root.
12. Open Active Swaps and confirm the active state.
13. Preview and confirm restore only against the sandbox root.
14. Confirm Backups and Diagnostics load.
15. Confirm Logs show the expected command sequence.

## Notes

- No updater automation is included in Phase 6.
- No signing is included in Phase 6.
- Restore before any normal or online Rocket League play.