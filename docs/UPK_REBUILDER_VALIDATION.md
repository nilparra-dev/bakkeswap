# UPK Rebuilder Validation

## Purpose

This document records the validation contract for the sandbox-only UPK rebuilder and the known-answer harness that exercises it.

The validation goal in this phase is not production install readiness. It is local proof that a rebuilt package can be emitted to a sandbox path, reparsed safely, and compared against source and expected outputs.

## Sandbox Rebuild Validation Contract

The sandbox writer returns `SandboxRebuildValidationResult`.

It records:

- `output_exists`: the rebuilt file was written and can be read back
- `filename_matches_target`: the rebuilt filename equals the target filename
- `output_parses`: the rebuilt package summary parses successfully
- `output_decrypts_tables`: the encrypted header-table region decrypts successfully
- `output_decompresses`: the Rocket League chunk body decompresses successfully
- `body_equals_source`: the rebuilt decompressed body hash matches the source package body hash
- `target_name_present`: the target identity exists in the rebuilt NameTable
- `target_export_name_count`: number of export object-name refs that now resolve to the target identity
- `modified_export_indices`: export indices the rebuild logic intentionally rewrote
- `warnings`: non-fatal validation findings
- `passed`: true only when all required checks succeed and the target export ref count covers the modified export refs

## Known-Answer Validation Contract

The known-answer harness returns `RebuildValidationSummary` and, when output generation is enabled, also includes the sandbox rebuild report.

The known-answer validation tracks:

- `source_body_matches_output_body`: whether the compared output body hash matches the source body hash
- `target_identity_present`: whether the compared output clearly exposes the target identity
- `modified_export_refs_detected`: whether generated output exposes at least as many target export refs as the rebuild reported modifying
- `byte_comparison`: optional byte-for-byte comparison against an expected package
- `warnings`: non-fatal comparison findings

## Synthetic Coverage In Phase 3C

The current automated coverage includes:

- sandbox rebuild success against synthetic source and target packages
- refusal when the output parent is missing and directory creation is not enabled
- refusal when the output path targets the configured cooked directory
- known-answer generation when `--output` is provided
- exact byte comparison when an expected sandbox output is provided

## Optional Local-Only Smoke Workflow

For developer-owned local samples only:

1. Place source, target, and optional expected packages under `local_samples/`.
2. Emit sandbox output under `local_output/`.
3. Run `bakkeswap upk rebuild-sandbox --source local_samples/source.upk --target local_samples/target.upk --output local_output/rebuilt_target.upk --json`.
4. Run `bakkeswap upk known-answer --source local_samples/source.upk --target local_samples/target.upk --expected local_samples/expected.upk --output local_output/rebuilt_target.upk --json`.
5. Run `bakkeswap upk inspect local_output/rebuilt_target.upk --json`.

This workflow must remain offline and must not target the real Rocket League install.
