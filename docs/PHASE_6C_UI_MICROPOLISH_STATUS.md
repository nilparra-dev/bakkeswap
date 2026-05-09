# Phase 6C UI Micro-Polish Status

## Goal

Phase 6C focused on presentation-only follow-up after the Phase 6B desktop-tool shell pass:

- tighten small wording and spacing issues in the Tauri shell
- make the install and restore surfaces read more clearly as confirmation tools
- capture a permanent sandbox-only screenshot set under `docs/screenshots/`
- refresh beta-facing docs so the local Windows beta has presentation collateral

No backend workflow changes were introduced in this phase.

## UI Changes Landed

- shortened and humanized shell badge wording such as `Sandbox-safe`, `Live-install risk`, and `Preview ready`
- switched visible path rendering to sandbox-aware clipped tails so long Windows paths stay readable in dense tables and screenshots
- added explicit next-step guidance to Quick Swap after TARGET and SOURCE selection
- made Install Preview and Restore read more like confirmation dialogs with simple step language
- tightened empty-state copy and operational button labels
- gave the Logs page terminal-style `[RUN]`, `[OK]`, and `[ERR]` state markers
- removed persistent horizontal mini-scrollbars from normal path cells

## Permanent Screenshot Set

The current sandbox-only screenshot gallery now lives under `docs/screenshots/`:

- `home.png`
- `game_folder.png`
- `quick_swap.png`
- `install_preview.png`
- `active_swaps.png`
- `backups.png`
- `diagnostics.png`
- `logs.png`

The screenshots were captured from the live Tauri app against `target/gui_smoke` only.

## Capture Workflow

1. Start the app from the repository root with `BAKKESWAP_APP_HOME=target/gui_smoke/app_home`.
2. Set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9224`.
3. Run `./scripts/capture_docs_screenshots.ps1`.
4. Verify the output files under `docs/screenshots/`.

## Validation

- `npm run check`
- live `npm run tauri:dev` against `target/gui_smoke/app_home`
- `./scripts/capture_docs_screenshots.ps1`

## Boundaries

- UI, docs, and screenshots only
- sandbox or project-local data only
- no real Rocket League path during capture or validation
- no committed `.upk` files or dumps
- offline and local only