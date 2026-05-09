# Phase 3A UPK Read-Only Status

Date: 2026-05-09

## Goal

Phase 3A ports the safe read-only `.upk` inspection foundation into Rust before any rebuild, package writing, install, or restore logic is implemented.

Status: complete and validated for the read-only scope.

## Implemented In This Pass

### New UPK read-only modules

- `format.rs`
- `reader.rs`
- `compression.rs`
- `tables.rs`
- `names.rs`
- `imports.rs`
- `exports.rs`
- `validate.rs`
- `inspect.rs`

### Read-only inspection behavior

- reads package magic, file version, and licensee version
- parses the package summary/header fields required by the old Python prototype
- detects the Rocket League UE3 package profile heuristically
- detects compression flags
- locates and parses Rocket League compressed chunk metadata from the decrypted header region
- decompresses the compressed body using zlib
- computes file SHA-256
- computes decompressed body SHA-256
- extracts string and name evidence for inspection output

### Table parsing

When the decrypted header region is valid, the Rust code now parses:

- NameTable
- ImportTable
- ExportTable
- DependsTable

This phase remains strictly read-only. No table writing, no re-encryption for rebuilds, and no offset rewriting were added.

### CLI

- `bakkeswap upk inspect <path>`
- `bakkeswap upk inspect <path> --json`

Default output is human-readable. `--json` returns a machine-readable inspection report.

## Safe Test Coverage

- binary reader unit tests using synthetic byte buffers
- compression and decompression tests using generated zlib bytes
- synthetic package inspection test covering:
  - summary parsing
  - table decryption
  - compressed chunk parsing
  - body decompression

No real `.upk` files are committed in the repository.

## Manual Sample Support

- `.gitignore` now excludes `local_samples/`
- `.gitignore` already excludes `*.upk`
- manual examples can use local-only ignored samples such as `local_samples/example.upk`

## Validation Result

The Rust validation set passes with the Phase 3A code included:

1. `cargo fmt`
2. `cargo check -p bakkeswap-core -p bakkeswap-cli`
3. `cargo test -p bakkeswap-core`
4. `cargo clippy -p bakkeswap-core -p bakkeswap-cli --all-targets -- -D warnings`

## Still Deferred

- target-identity rebuild logic
- package writing
- install logic
- restore logic
- any modification of `CookedPCConsole`
- runtime hooks or online behavior

## Safety Boundary

This phase is read-only only:

- no rebuilding
- no writing packages
- no install
- no restore
- no real game-folder modification
- offline/local only