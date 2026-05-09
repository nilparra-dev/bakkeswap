# GUI UX Flow

## Principles

- TARGET is the item the player already owns or equips
- SOURCE is the item the player wants to see locally
- the GUI is a control surface over Rust services, not a second implementation of swap logic
- preview results and confirmation phrases come from the backend and remain the source of truth
- path posture in the GUI is advisory only: sandbox or project-local, local custom, or possible live install

## Startup

1. On launch, the app loads status, config, installed swaps, and permanent-original backup status.
2. If Tauri is unavailable, the frontend enters browser-only mode and shows that desktop actions are disabled.
3. The Home page summarizes the current safety posture before the user starts planning or mutating files.

## Home

- show configured `CookedPCConsole` summary
- show the current path posture clearly so sandbox roots do not look like live installs
- show indexed local file count, database counts, active swaps, and backup counts
- keep offline-only and sandbox-first rules visible
- allow a top-level refresh of backend state

## Game Folder

1. User pastes a path or picks a local folder.
2. Backend validation checks whether the path points at a Rocket League root, `TAGame`, or `CookedPCConsole`.
3. The GUI clears stale validation as soon as the input changes.
4. The GUI renders blockers, warnings, normalized path, sample files, and path posture.
5. Only after validation does the user persist the path.

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
5. Before plan creation, the GUI shows a lightweight same-slot metadata preflight, while keeping the backend as the source of truth.
6. After both sides are selected, the user creates a plan and then builds it.

## Install Preview

1. The user requests install preview for the current built plan.
2. The backend returns blockers, warnings, file actions, and the exact confirmation phrase.
3. The risky screen keeps the configured `CookedPCConsole` path visible at all times.
4. The GUI keeps the confirm action disabled until:
   - preview status is ready
   - blockers are empty
   - the user types the exact backend-issued phrase
5. If execution still returns a blocked install report, the GUI stays on Install Preview and surfaces the execution blockers instead of redirecting away.
6. The confirmation phrase is rendered prominently instead of being hidden in supporting text.
7. Optional overwrite of an existing profile backup remains an explicit checkbox, not a silent default.

## Active Swaps And Restore

1. The Active Swaps page lists installed profiles from local manifests and SQLite.
2. User selects one profile and requests restore preview.
3. The backend returns restore blockers, warnings, and the exact confirmation phrase.
4. The risky screen shows both the selected install root and the currently configured `CookedPCConsole`.
5. Standard restore stays on the per-profile backup path.
6. Restore from permanent originals is an explicit emergency toggle and must remain visibly distinct.

## Backups

- show permanent-original backup tracking counts
- show missing-file counts clearly
- show original and profile backup roots clearly
- summarize backup health in plain language, not just raw counters
- provide explicit verification on demand

## Diagnostics

- show app home path
- show database path
- show configured `CookedPCConsole`
- show indexed file count, product count, title count, and installed swap count
- show backup root paths
- clarify that logs remain session-local and in-memory for the current desktop session

## Logs

- show recent command start, success, and failure entries
- keep the log local to the current app session
- use it to explain backend activity without exposing a second command surface to the user
- show the expected sandbox smoke command sequence so a human click-through can be checked against it quickly