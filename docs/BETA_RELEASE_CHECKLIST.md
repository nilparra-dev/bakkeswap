# Beta Release Checklist

## Package Safety

- confirm the app is still local-only and offline-only
- confirm no runtime hooks, overlays, or anti-cheat bypass behavior were introduced
- confirm no real Rocket League path was used during validation
- confirm no `.upk` files are included in the beta folder
- confirm no CodeRed dumps are included in the beta folder
- confirm no personal machine paths are written into beta collateral

## Build

- run `npm run check`
- run `npm run build`
- run `npm run tauri:build`
- confirm Windows bundle artifacts exist

## Release Folder

- recreate `dist/beta/BakkesSwap-v0.1.0-beta/` after frontend builds because `npm run build` refreshes `dist/`
- copy Windows bundle artifacts into the folder
- include `README.md`
- include `OFFLINE_ONLY_WARNING.md`
- include `docs/SAFETY_MODEL.md`
- include `docs/GUI_UX_FLOW.md`
- include `docs/UI_STYLE_GUIDE.md`
- include `docs/PHASE_6C_UI_MICROPOLISH_STATUS.md`
- include `docs/screenshots/` if you want screenshot-backed beta collateral
- include `docs/BUILDING_FROM_SOURCE.md` if useful for testers
- include `QUICK_START.md`

## Smoke Validation

- launch the packaged app
- load Home
- validate the fake Game Folder path
- import fake dump metadata
- refresh DB
- search TARGET and SOURCE
- create plan
- build plan
- open install preview
- confirm install only against the sandbox root
- confirm restore only against the sandbox root
- confirm Backups loads as ready
- confirm Diagnostics loads with sandbox-local paths
- confirm Logs shows the expected command sequence

## Screenshot Capture

- run the live sandbox app with `BAKKESWAP_APP_HOME=target/gui_smoke/app_home`
- expose WebView2 debugging with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9224`
- run `./scripts/capture_docs_screenshots.ps1`
- confirm `docs/screenshots/` contains the current Home, Game Folder, Quick Swap, Install Preview, Active Swaps, Backups, Diagnostics, and Logs shots
- confirm screenshots only show sandbox-local paths and no real machine state

## Optional Artifacts

- include CLI executable only if it is cleanly built and useful
- generate `SHA256SUMS.txt` if practical

## Final Status

- mark the build as a beta candidate, not a stable public release
- remind testers to restore before normal or online play