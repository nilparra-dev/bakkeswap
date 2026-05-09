# Migration Plan

## Objective

Move from the working Python prototype to a professional Rust + Tauri + SQLite application without losing any validated behavior or safety guarantees.

## Reference State To Preserve

- target-identity rebuild is the working method
- raw rename or raw copy failed and must not become the default path
- CodeRed import path exists and produced 8955 products in a validated run
- real game path handling exists
- permanent original backups exist
- per-profile backups exist
- install and restore safety model exists
- GUI semantics already distinguish TARGET from SOURCE

## Phase 0. Freeze The Reference

Deliverables:

- keep Python source intact
- create migration workspace under `bakkeswap/`
- create `legacy_python/README.md` index
- document the required golden tests and module mapping

Exit criteria:

- old app still usable
- new workspace isolated

## Phase 1. Core `.upk` Foundations

Deliverables:

- parser skeleton
- encrypted table handling skeleton
- name, import, export, and depends table models
- target-identity rebuilder skeleton
- validator skeleton

Exit criteria:

- Rust crate structure exists
- architecture and schema docs are agreed

## Phase 2. Database And File Discovery

Deliverables:

- SQLite migration runner
- CodeRed import implementation
- local file discovery and hashing
- search implementation

Exit criteria:

- metadata import parity can be measured against the Python app
- local files can be indexed without touching real installs

## Phase 3. Planner And Builder Parity

Deliverables:

- TARGET/SOURCE planner implementation
- build record persistence
- rebuild validator implementation

Exit criteria:

- golden build cases pass in a sandbox
- body equality and target-identity assertions are enforced

## Phase 4. Install, Restore, And Backups

Deliverables:

- permanent original backup manager
- per-profile backup manager
- install dry-run preview
- restore implementation

Exit criteria:

- dry-run shows exact files and backups
- real install remains manual only
- restore returns sandbox state cleanly

## Phase 5. CLI Completion

Deliverables:

- full CLI command parity
- selftest
- status reporting
- backup verification

Exit criteria:

- CLI is the authoritative backend contract for the GUI
- golden test suite passes without real game-file writes

## Phase 6. Tauri GUI

Deliverables:

- Home
- Game Folder
- Quick Swap
- Active Swaps
- Backups
- Database
- Settings
- Logs

Exit criteria:

- GUI uses Rust services instead of re-implementing logic in frontend code
- install confirmation and dry-run safety remain obvious

## Phase 7. External Overlay, If Still Wanted

Deliverables:

- external always-on-top window
- F2 hotkey

Exit criteria:

- GUI is stable first
- no injection
- no memory access

## Explicit Non-Goals For v1

- no Player Title runtime module
- no online cheating features
- no server inventory changes
- no EAC bypass
- no injected overlay
- no runtime memory editing

## Current Status

This repo is at the start of Phase 0 and Phase 1:

- new workspace created
- CLI and Tauri shells created
- schema draft created
- implementation not ported yet