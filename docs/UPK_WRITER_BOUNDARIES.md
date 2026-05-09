# UPK Writer Boundaries

## Purpose

This document defines what the Rust UPK writer is allowed to do once implemented and, more importantly, what it is not allowed to do during the current rewrite phases.

## Current State

The repository now contains a writer planning surface in `writer.rs`, but real package generation remains disabled.

The current writer-related code may:

- derive a sandbox profile name
- derive a target output filename
- derive a sandbox output path
- describe a sandbox-only write plan

The current writer-related code may not:

- emit a modified `.upk`
- write into `CookedPCConsole`
- install into the Rocket League folder
- restore game files

## Hard Boundaries

These boundaries are mandatory:

- no real install
- no restore
- no modification of `CookedPCConsole`
- no committed `.upk` files
- no runtime hooks
- no EAC bypass
- offline/local only

## Sandbox Output Rule

If package generation is enabled in a later phase, it must only write under an explicit sandbox output root chosen by the developer.

It must not infer or silently default to the live game folder.

Examples of acceptable future destinations:

- `sandbox/output/`
- another developer-owned local scratch folder outside the game install

Examples of unacceptable destinations:

- Rocket League `CookedPCConsole`
- any live game-content folder
- any automatically discovered install path used without explicit sandbox intent

## Build Command Boundary

`bakkeswap build` must remain stubbed until all of the following are true:

1. sandbox-only writer primitives exist
2. known-answer validation exists for generated outputs
3. generated outputs can be compared against validated local samples
4. the command surface makes the sandbox-only boundary explicit

Until then, build must not be presented as production-ready.

## Required Validation Before Any Future Writer Enablement

Before any real sandbox output is considered acceptable, the writer must be able to prove:

- output filename equals the target filename
- source body is preserved in the output
- target identity is present in the rebuilt package
- export object-name rewrites are detected as expected
- serial offsets are updated consistently with header-size changes
- known-answer comparisons can explain byte divergence clearly

## Local Samples Rule

Developer `.upk` samples remain local-only:

- use `local_samples/`
- do not commit samples
- do not commit generated outputs

The repository should continue to rely on ignored local-only samples and synthetic tests until a sandbox-only writer is fully validated.