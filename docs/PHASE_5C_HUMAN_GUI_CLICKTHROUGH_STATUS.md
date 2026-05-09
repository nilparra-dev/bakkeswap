# Phase 5C Human GUI Click-Through Status

## Goal

Phase 5C validates the actual Tauri desktop window against the existing `target/gui_smoke` sandbox and fixes only small UX issues exposed by that live click-through.

This phase does not add packaging, release work, overlays, hooks, or backend feature expansion.

## Sandbox Used

The live desktop pass reused the existing Phase 5B sandbox:

- `target/gui_smoke/app_home`
- `target/gui_smoke/codered_dumps`
- `target/gui_smoke/RocketLeague/TAGame/CookedPCConsole`
- `target/gui_smoke/smoke_report.json`

The Tauri app was launched against that sandbox app home so no real Rocket League path or real `.upk` assets were involved.

## Live Flow Covered

The actual desktop window was exercised through:

- Home
- Game Folder
- Database
- Quick Swap
- Install Preview
- Active Swaps
- restore preview and confirmed restore
- Backups
- Diagnostics
- Logs

The live click-through confirmed that the UI clearly shows:

- TARGET as the item the player owns or equips
- SOURCE as the item the player wants to see locally
- the configured `CookedPCConsole` path
- sandbox or project-local path posture
- install confirmation phrase
- restore confirmation phrase
- permanent-original backup posture
- active versus inactive swap state

## Issues Found And Fixed

The real Tauri window exposed these frontend issues:

1. long sandbox paths caused horizontal overflow and a bottom scrollbar on the desktop window
2. the Game Folder nav badge stayed on `Needs setup` even when the sandbox path was already configured
3. the frontend `create_plan` invoke payload used the wrong argument keys for the Tauri command bridge
4. a blocked re-install navigated away from Install Preview even though the backend returned a blocked install report
5. install and restore file-action rows were cramped and hard to scan because status text and paths ran together
6. Diagnostics still said session logs were in-memory only in Phase 5B instead of using neutral current-session wording

Phase 5C fixed those issues by:

- hardening layout wrapping for long path values
- making nav badges explicitly reactive to current loaded state
- correcting the `create_plan` frontend invoke payload to use the command bridge keys actually expected by Tauri
- keeping blocked installs on the Install Preview screen and surfacing execution blockers there
- improving file-action row spacing and readability
- updating stale diagnostics copy

## Live Outcome

The live desktop pass against `target/gui_smoke` completed this sequence successfully:

- sandbox state loaded on Home
- Game Folder showed the saved fake Rocket League root and normalized `CookedPCConsole`
- Database showed the saved fake dump folder and indexed counts
- Quick Swap searched `Target Decal` and `Source Decal`
- plan creation succeeded for `source_decal_on_target_decal`
- build succeeded
- install preview showed `INSTALL source_decal_on_target_decal`
- the first install attempt correctly showed a blocked re-install because the profile backup already existed from the prior smoke run
- enabling overwrite allowed the install to succeed
- Active Swaps showed one active install record plus the older inactive record
- restore preview showed `RESTORE source_decal_on_target_decal`
- confirmed restore succeeded
- Active Swaps returned to `0 active`
- Backups still reported `ready`
- Diagnostics showed local sandbox paths and counts
- Logs showed the expected install and restore command sequence, including the blocked re-install and the later successful install

## Safety Boundary

Phase 5C stayed inside the same safety boundary as Phase 5B:

- local-only and offline-only
- no real Rocket League install path
- no real `.upk` assets
- no runtime hooks
- no anti-cheat bypass behavior
- no packaging or updater work
- no overlay or F2 work

## Validation

The required validation gate after the Phase 5C UI changes is:

- `npm run check`
- `npm run build`
- `cargo check -p bakkeswap-tauri`
- `cargo fmt`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
- `cargo test -p bakkeswap-tauri gui_sandbox_smoke::controlled_gui_sandbox_smoke_flow -- --exact --nocapture --test-threads=1`