# GUI UX Flow

## Principles

- TARGET is the item the player already owns or equips
- SOURCE is the item the player wants to see locally
- the GUI is a control surface over Rust services, not a second implementation of swap logic
- preview results and confirmation phrases come from the backend and remain the source of truth

## Startup

1. On launch, the app loads status, config, installed swaps, and permanent-original backup status.
2. If Tauri is unavailable, the frontend enters browser-only mode and shows that desktop actions are disabled.
3. The Home page summarizes the current safety posture before the user starts planning or mutating files.

## Home

- show configured `CookedPCConsole` summary
- show indexed local file count, database counts, active swaps, and backup counts
- keep offline-only and sandbox-first rules visible
- allow a top-level refresh of backend state

## Game Folder

1. User pastes a path or picks a local folder.
2. Backend validation checks whether the path points at a Rocket League root, `TAGame`, or `CookedPCConsole`.
3. The GUI renders blockers and warnings returned by the backend.
4. Only after validation does the user persist the path.

## Database

1. User selects the folder containing `ProductDump.json` and related dump files.
2. Import runs through the backend importer.
3. Refresh updates the local file index and status counts.
4. Warnings and summaries stay visible so the user can tell whether the workspace is ready for search.

## Quick Swap

1. User searches TARGET products.
2. User searches SOURCE products.
3. Search is debounced and limited so the desktop shell stays responsive.
4. Only swappable product hits are selectable.
5. After both sides are selected, the user creates a plan and then builds it.

## Install Preview

1. The user requests install preview for the current built plan.
2. The backend returns blockers, warnings, file actions, and the exact confirmation phrase.
3. The GUI keeps the confirm action disabled until:
   - preview status is ready
   - blockers are empty
   - the user types the exact backend-issued phrase
4. Optional overwrite of an existing profile backup remains an explicit checkbox, not a silent default.

## Active Swaps And Restore

1. The Active Swaps page lists installed profiles from local manifests and SQLite.
2. User selects one profile and requests restore preview.
3. The backend returns restore blockers, warnings, and the exact confirmation phrase.
4. Standard restore stays on the per-profile backup path.
5. Restore from permanent originals is an explicit emergency toggle and must remain visibly distinct.

## Backups

- show permanent-original backup tracking counts
- show missing-file counts clearly
- provide explicit verification on demand

## Logs

- show recent command start, success, and failure entries
- keep the log local to the current app session
- use it to explain backend activity without exposing a second command surface to the user