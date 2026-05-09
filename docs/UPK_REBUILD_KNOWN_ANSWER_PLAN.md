# UPK Rebuild Known-Answer Plan

## Purpose

The known-answer harness exists to compare Rust rebuild behavior against Python-validated target-identity swaps without enabling install or production build flows.

It is intended to answer:

- does the rebuilt package preserve the source body?
- does the rebuilt package expose the target identity?
- do header/table characteristics stay in the expected range?
- if a generated sandbox output exists later, where does it diverge byte-for-byte from a validated known-answer package?

## Current Rust Command

The current safe CLI surface is:

- `bakkeswap upk known-answer --source <path> --target <path> --expected <path>`
- `bakkeswap upk known-answer --source <path> --target <path> --expected <path> --json`
- `bakkeswap upk known-answer --source <path> --target <path> --output <path>`
- `bakkeswap upk known-answer --source <path> --target <path> --expected <path> --output <path> --json`

Current behavior is read-only unless an explicit sandbox output path is provided:

- inspect source package
- inspect target package
- inspect expected known-answer package when provided
- generate a sandbox-only rebuilt output when `--output` is provided
- validate that the generated output parses, decrypts, decompresses, preserves the source body, and exposes the target identity
- compare decompressed body hashes when available
- compare table-count snapshots
- derive the planned profile name and output filename for sandbox generation

Current non-behavior:

- no install
- no restore
- no writing to the game folder

## Known Python-Validated Cases

These are the known target/source pairs to preserve in the Rust harness workflow:

1. Affluenter on Unzipped
   target `4916`
   source `7598`
2. Contagion on Standard banner
   target `2526`
   source `2479`
3. Cosmological on Bubbly
   target `1888`
   source `11330`
4. Lunation on 20XX
   target `1684`
   source `7856`
5. Laser Wave III on Gaseous
   target `10708`
   source `3224`

## Local Sample Workflow

Developer sample placement remains local-only:

- place packages under `local_samples/`
- do not commit those samples
- keep relying on `.gitignore` for both `local_samples/` and `*.upk`

Example local workflow:

1. place source, target, and expected known-answer `.upk` files under `local_samples/`
2. run the known-answer command against those local-only paths
3. keep all generated or temporary outputs inside a separate sandbox-only folder when generation is enabled

## Current Comparison Model

The current harness can already report:

- source package inspect summary
- target package inspect summary
- expected package inspect summary
- source identity and target identity derivation
- target identity candidate list
- body hash match or mismatch
- table-count snapshots for source, target, expected, and later generated output

The current API supports an optional generated-output path. When provided, the harness writes a sandbox-only rebuilt package there and can compare it byte-for-byte against the expected known-answer package while reporting the first divergence offset.

## Acceptance Targets For Future Sandbox Output

When sandbox generation exists, the known-answer harness should verify:

- output filename matches the target package filename
- decompressed output body hash equals the source body hash
- target identity is present in the rebuilt package
- modified export refs are detected where expected
- byte comparison against the expected known-answer package is explainable when not exact

## Safety Boundary

The known-answer harness is for offline local validation only. It must not become an install surface or a direct write path into Rocket League directories.