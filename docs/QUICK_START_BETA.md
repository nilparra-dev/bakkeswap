# Quick Start Beta

## Before You Run It

BakkesSwap beta is local-only and offline-only.

- do not use it online
- do not use it with anti-cheat enabled expectations
- do not point it at a real Rocket League install during beta smoke validation
- restore before normal or online play

Other players do not see your local file changes.

## Fast Start

1. Start `BakkesSwap`.
2. On Home, confirm the app opens normally.
3. Open Game Folder.
4. Point it at a copied or fake Rocket League root such as `target/gui_smoke/RocketLeague`.
5. Validate the path and save it.
6. Open Database.
7. Import a fake dump folder such as `target/gui_smoke/codered_dumps`.
8. Refresh database.
9. Open Quick Swap.
10. Search TARGET `Target Decal` or `1001`.
11. Search SOURCE `Source Decal` or `1002`.
12. Create a plan and build it.
13. Open Install Preview, review the affected files, and read the confirmation phrase.
14. Only confirm install against the sandbox path.
15. Open Active Swaps and use restore preview and restore confirmation against the same sandbox path.

## Screenshot Reference

Use the sandbox screenshot set under `docs/screenshots/` as the expected visual reference for the current beta shell.

- `screenshots/home.png`
- `screenshots/game_folder.png`
- `screenshots/quick_swap.png`
- `screenshots/install_preview.png`
- `screenshots/active_swaps.png`
- `screenshots/backups.png`
- `screenshots/diagnostics.png`
- `screenshots/logs.png`

## What To Watch For

- TARGET means the item you already own or equip
- SOURCE means the item you want to see locally
- the configured `CookedPCConsole` path should stay visible on risky screens
- path posture should make it obvious when the app is pointing at a sandbox path
- install and restore should require the exact confirmation phrase
- Backups should remain ready
- Diagnostics and Logs should load without errors

## If Something Looks Wrong

- stop before install or restore if the path is not clearly sandboxed
- stop if the UI shows blockers you do not understand
- stop if the app points outside your fake or copied root
- do not move on to real game files during beta validation