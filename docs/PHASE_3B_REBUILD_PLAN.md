# Phase 3B Rebuild Plan

Date: 2026-05-09

## Goal

Phase 3B prepares the Rust target-identity rebuild implementation without enabling production package output, install, or restore flows.

Status: planning surface and pure helper foundation implemented. Real package writing remains disabled.

## Scope Added In This Pass

### New modules

- `crates/bakkeswap-core/src/upk/rebuild.rs`
- `crates/bakkeswap-core/src/upk/writer.rs`
- `crates/bakkeswap-core/src/upk/known_answer.rs`
- `crates/bakkeswap-core/src/upk/validation.rs`

### Safe code added

- rebuild pipeline stage model for the future write path
- filename identity extraction helpers
- target identity candidate derivation helpers
- sandbox profile and output filename resolution helpers
- export object-name reference matching helpers
- serial offset delta and projection helpers
- known-answer harness report types and read-only analysis path
- byte comparison and table-count snapshot helpers

### CLI added

- `bakkeswap upk known-answer --source <path> --target <path> --expected <path>`
- `--json` supported

This command is read-only in the current phase. It does not generate output packages and does not touch the Rocket League install.

## Rebuild Pipeline Design

The intended target-identity rebuild pipeline remains:

1. read the source package
2. read the target package
3. preserve the source-derived body
4. append or ensure the target identity name in the rebuilt name data
5. update selected export object-name references from source identity to target identity
6. recalculate export serial offsets if header size changes
7. rebuild header tables
8. re-encrypt the header table region
9. recompress and re-emit chunk data
10. emit sandbox-only output with the target filename
11. validate the output package

## What Is Implemented Now

### Pure helpers only

The Rust code now safely supports the low-risk helper layer needed before the real rebuild writer:

- identity extraction from package filenames
- target identity candidate generation from target filenames
- planned sandbox profile name resolution
- planned target output filename resolution
- planned sandbox output path resolution
- export object-name reference matching against source identity candidates
- serial offset delta calculation and projection

### Known-answer harness

The new harness currently inspects:

- source package
- target package
- expected known-answer package when provided

It reports:

- source, target, and expected inspect summaries
- source and target identity derivation
- target identity candidates
- body-hash comparison when an expected output is available
- table-count snapshots
- warnings about missing expected packages or mismatched known-answer traits

The API also includes optional future fields for sandbox output comparison, but generation is still disabled.

## Safe Tests Added

- identity derivation tests
- target candidate derivation tests
- sandbox profile and output filename resolution tests
- export object-name reference matching tests using fake export data
- serial offset delta projection tests
- byte-difference helper tests
- known-answer identity detection tests using fake inspect reports

No real `.upk` files are committed for these tests.

## Explicit Deferrals

This phase does not enable any of the following:

- production package building
- install
- restore
- writing into `CookedPCConsole`
- runtime hooks
- online behavior
- EAC bypass or similar behavior

## Next Logical Implementation Slice

When Phase 3C begins, the recommended next steps are:

1. implement header table rebuild primitives in sandbox-only mode
2. implement header re-encryption and chunk re-emission in sandbox-only mode
3. emit output only under an explicit sandbox output root
4. validate generated output against known-answer packages before any broader build wiring