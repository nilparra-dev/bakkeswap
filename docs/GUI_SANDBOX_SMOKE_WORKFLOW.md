# GUI Sandbox Smoke Workflow

## Goal

This workflow validates the Phase 5B desktop flow using only fake metadata, fake package files, and a sandboxed fake `CookedPCConsole`.

It must never point at a real Rocket League install.

## Sandbox Layout

The controlled helper creates and refreshes this tree under the repository root:

- `target/gui_smoke/app_home`
- `target/gui_smoke/codered_dumps`
- `target/gui_smoke/RocketLeague/TAGame/CookedPCConsole`
- `target/gui_smoke/smoke_report.json`

The helper copies the fake CodeRed planner fixtures and generates synthetic `.upk`-like files that the Rust rebuild path accepts.

## Controlled Helper

From the repository root, run:

1. `cargo test -p bakkeswap-tauri gui_sandbox_smoke::controlled_gui_sandbox_smoke_flow -- --exact --nocapture --test-threads=1`

This command:

- deletes any previous `target/gui_smoke`
- recreates the sandbox directory tree
- copies the fake dump fixtures from `crates/bakkeswap-core/tests/fixtures/codered_planner`
- generates synthetic package files for:
  - `Skin_Target_SF.upk`
  - `Skin_Target_T_SF.upk`
  - `Skin_Source_SF.upk`
  - `Skin_Source_T_SF.upk`
  - `Antenna_Source_SF.upk`
- points `BAKKESWAP_APP_HOME` at `target/gui_smoke/app_home`
- drives the Tauri command bridge through the same flow the GUI uses
- writes a machine-readable summary to `target/gui_smoke/smoke_report.json`

## Expected Controlled Result

The current passing smoke report records:

- `import_summary_products = 4`
- `refresh_indexed_files = 5`
- `plan_profile = source_decal_on_target_decal`
- `plan_same_slot = true`
- `build_status = built`
- `install_preview_status = preview_ready`
- `install_status = installed`
- `active_swaps_after_install = 1`
- `restore_preview_status = preview_ready`
- `restore_status = restored`
- `active_swaps_after_restore = 0`
- `inactive_swaps_after_restore = 1`
- `backup_status = ready`
- `backup_verify_status = ready`

## Manual GUI Walkthrough

After the controlled helper succeeds:

1. Start the app with `npm run tauri:dev`.
2. Open the Game Folder page.
3. Set the path to `target/gui_smoke/RocketLeague`.
4. Run validation and confirm the normalized path resolves to `target/gui_smoke/RocketLeague/TAGame/CookedPCConsole`.
5. Confirm the path posture is shown as sandbox or project-local, not as a live install.
6. Open the Database page.
7. Import `target/gui_smoke/codered_dumps`.
8. Refresh the database and local file index.
9. Confirm the UI reports 4 products and 5 indexed local `.upk` files.
10. Open Quick Swap.
11. Search TARGET for `Target Decal` and select product `1001`.
12. Search SOURCE for `Source Decal` and select product `1002`.
13. Confirm the preflight section shows matching slot metadata.
14. Create the backend plan.
15. Confirm the plan profile is `source_decal_on_target_decal` and backend compatibility remains same-slot.
16. Build the plan.
17. Open Install Preview.
18. Confirm the configured CookedPCConsole path shown on the screen still points at the sandbox path.
19. Confirm the preview status is ready and the confirmation phrase is `INSTALL source_decal_on_target_decal`.
20. Type the exact phrase and confirm install.
21. Open Active Swaps and confirm the profile appears as active.
22. Preview restore.
23. Confirm the restore phrase is `RESTORE source_decal_on_target_decal`.
24. Type the exact restore phrase and confirm restore.
25. Confirm the profile record remains listed but becomes inactive.
26. Open Backups and confirm permanent originals remain ready and verified.
27. Open Logs and confirm the command sequence includes:
   - `validate_game_path`
   - `set_game_path`
   - `import_codered`
   - `refresh_db`
   - `search_items`
   - `create_plan`
   - `build_plan`
   - `install_preview`
   - `install_confirmed`
   - `restore_preview`
   - `restore_confirmed`
   - `backup_originals_verify`

## Notes

- The controlled helper validates the Tauri bridge and backend contract path automatically.
- The manual GUI walkthrough is still useful for visual confirmation of labels, disabled states, warnings, and recent command logs.
- If the sandbox needs to be regenerated, rerun the helper command. It recreates `target/gui_smoke` from scratch.