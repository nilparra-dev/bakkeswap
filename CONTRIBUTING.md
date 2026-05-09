# Contributing

## Repository Purpose

This repository is the public Rust + Tauri rewrite workspace for BakkesSwap.

Keep contributions focused on:

- Rust core architecture
- SQLite-backed metadata and state
- CLI parity with the documented contract
- Tauri and Svelte GUI scaffolding
- sandboxed validation only

## Hard Boundaries

Do not add or propose:

- EAC bypass behavior
- online cheating behavior
- server inventory mutation
- runtime memory editing
- injected overlays in v1
- Player Title runtime work in v1
- raw rename or raw copy as the default swap method

## Public Repo Hygiene

Never commit any of the following:

- Rocket League game files
- `.upk` files
- generated modified packages
- CookedPCConsole copies
- personal CodeRed dumps
- inventory dumps
- backup folders
- logs with personal paths or tokens
- secrets, auth tokens, or `.env` files
- local absolute machine paths when avoidable

Use generic examples such as:

- `C:\Path\To\RocketLeague\TAGame\CookedPCConsole`
- `D:\Games\rocketleague\TAGame\CookedPCConsole`

## Development Rules

- preserve the offline-only safety model
- keep automated tests sandboxed
- never target the real game install during automated validation
- prefer explicit dry-run previews for anything file-destructive
- keep exact filename and capitalization behavior in scope for the eventual rebuilder port

## Python Reference

The Python prototype is not part of this public repository.

When porting behavior, reference it conceptually through the migration docs and private local development material, not by copying unrelated legacy content into this repo.

## Pull Request Guidance

- explain which rewrite phase the change belongs to
- mention any golden test or migration doc impacted
- call out any safety-sensitive behavior explicitly
- keep changes narrowly scoped where possible