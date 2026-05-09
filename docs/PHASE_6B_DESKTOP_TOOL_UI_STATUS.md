# Phase 6B Desktop Tool UI Status

## Goal

Phase 6B reshapes the Tauri shell so BakkesSwap feels like a compact desktop utility or mod menu instead of a generic web page.

This phase is UI-only. Backend behavior, Rust service boundaries, and safety constraints remain unchanged.

## Direction

- dark utility-panel shell
- fixed left sidebar on desktop
- compact top status bar
- dense panels and tables
- compact status badges
- terminal-like logs
- clear confirmation windows for install and restore

## Implemented

- replaced the large hero-style shell header with a compact top status bar
- tightened spacing, typography, radii, and control density across the app
- kept left-sidebar navigation and converted page badges into compact status chips
- split Quick Swap into TARGET and SOURCE utility columns with selected-item cards
- reworked Install Preview into a compact confirmation tool window with file and backup tables
- converted Active Swaps into a compact full-width table with a visible per-row restore action
- rendered Diagnostics as compact key/value tables
- rendered Logs as a dark monospace log panel with a copy action
- added reusable CSS primitives for panels, status badges, tables, path text, and logs

## Live Sandbox Validation

The UI pass was checked against the same fake sandbox boundary already used for Phase 5B and Phase 5C.

Environment:

- `BAKKESWAP_APP_HOME=D:\rl-item-changer\bakkeswap\target\gui_smoke\app_home`
- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9224`
- `npm run tauri:dev`

Live checks completed:

- Home reads as a compact desktop utility shell
- long `CookedPCConsole` paths remain readable in the top bar and tool panels
- Quick Swap clearly separates TARGET and SOURCE and keeps selected-item metadata compact
- Install Preview reads like a confirmation tool window, not a web page
- Active Swaps shows the restore table and per-row restore action clearly
- Logs renders in a terminal-like panel and retains a copy action

## Validation Commands

- `npm run check`
- `npm run build`
- `cargo check -p bakkeswap-tauri`
- `cargo fmt --check`
- `cargo check -p bakkeswap-core -p bakkeswap-cli`
- `cargo test -p bakkeswap-core`
- `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`
- `cargo test -p bakkeswap-tauri gui_sandbox_smoke::controlled_gui_sandbox_smoke_flow -- --exact --nocapture --test-threads=1`

## Boundaries

- UI polish only
- no F2 overlay
- no runtime hooks
- no real Rocket League path during validation
- no committed `.upk` files
- offline and local only
