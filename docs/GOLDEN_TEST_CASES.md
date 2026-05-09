# Golden Test Cases

## Rule For All Automated Tests

- never modify the real `CookedPCConsole`
- always use a sandbox or copied install path
- no real install during automated tests
- preserve exact filenames and capitalization
- keep all safety warnings visible in logs or test output

## Phase 2 Planner Gate

Before rebuild-oriented golden cases can be treated as release gates, the planner foundation must pass these fake-fixture checks:

1. successful same-slot plan
2. missing target product error
3. missing source product error
4. slot mismatch blocker
5. missing target visual package blocker
6. missing source visual package blocker
7. thumbnail missing warning without blocking a valid visual-only plan
8. Player Title product blocked as non-swappable

These checks now run from safe fake metadata fixtures plus temporary runtime-created `.upk` files only.

## Shared Assertions For Every Golden Swap Case

Each golden case must prove all of the following:

1. plan resolves the correct source and target UPKs
2. build passes validation
3. output body equals source body
4. target identity is present in the rebuilt output
5. modified export refs are detected
6. install dry-run shows exact target files and backup files
7. no real install occurs during automated tests

## Phase 3D Saved-Plan Build Gate

Before local known-answer samples become release gates, the saved-plan build service must pass these synthetic checks:

1. visual-only plan builds to the default workspace build root
2. visual plus thumbnail plan builds to an explicit sandbox output root
3. blocked plan returns a blocked build report and writes no outputs
4. output roots inside `CookedPCConsole` are rejected
5. missing source package paths fail with a helpful blocker message
6. missing target package paths fail with a helpful blocker message
7. plan JSON is updated with build status and validation results

## Phase 4A Install Dry-Run Gate

Before any real install path exists, the install preview service must pass these synthetic checks:

1. visual-only saved-plan preview reports the default install target and no files are modified
2. visual plus thumbnail saved-plan preview reports both install targets and both backup paths
3. missing built visual output returns a blocked preview
4. missing CookedPCConsole destination file returns a blocked preview
5. destination hash drift produces an explicit warning without writing files

## Phase 4C Install Execution Gate

Before real local install is considered usable, the confirmed install path must pass these synthetic checks:

1. missing confirmation returns a blocked install report and writes nothing
2. wrong confirmation returns a blocked install report and writes nothing
3. preview blockers prevent install execution and leave destination files unchanged
4. visual-only install copies the rebuilt output into the sandbox cooked root and creates both backup sets
5. visual plus thumbnail install updates both destination files and records both backup sets
6. existing profile backup blocks install unless overwrite is explicitly enabled
7. overwrite-enabled reinstall preserves the permanent original backup without replacing it
8. `install_manifest.json` is written under `workspace/backups/<profile_name>/`
9. saved plan JSON is updated with `install_status` and `last_install`
10. `installed_swaps` is written when the plan exists in SQLite
11. unsafe target filenames that attempt path escape are blocked

## Phase 4D Restore Gate

Before restore is considered usable, the confirmation-gated restore path must pass these synthetic checks:

1. restore dry-run reports exact files, destinations, backup paths, and hash state without writing files
2. successful profile restore copies files back from `workspace/backups/<profile_name>/`
3. wrong confirmation blocks restore
4. missing confirmation blocks restore
5. missing profile backup files block restore
6. tampered profile backup hashes block restore
7. path escape attempts in restore manifests are blocked
8. restored destination hashes are verified after copy
9. `install_manifest.json` is updated with `restored_at` when it exists
10. saved plan JSON is updated to `install_status = restored` when available
11. `installed_swaps` is updated with `restored_at` and `active = 0` when the plan exists in SQLite
12. original-backup fallback is refused unless `--from-originals` and `RESTORE ORIGINALS <profile_name>` are both present
13. emergency original-backup fallback succeeds in sandbox when the profile backup is missing but originals are still valid

## Phase 4B Backup Manager Gate

Before any real install path exists, the backup managers must pass these synthetic checks:

1. permanent original backup creates files and `manifest.json`
2. permanent original backup second run does not overwrite an existing backup
3. permanent original backup verify reports tracked files as valid when hashes still match
4. permanent original backup hash mismatch returns a blocker
5. profile backup creates files and `manifest.json`
6. existing profile backup folders refuse reuse by default
7. profile backup overwrite verification blocks on a tampered existing backup
8. backup preparation never installs rebuilt files and never modifies destination files

## 1. Boost Known-Answer Rebuild

Purpose:

- preserve the known-answer rebuild baseline from the Python research path

Reference assets:

- `samples/Boost_Bubble_SF.upk`
- `samples/Boost_Bubble_SF_modified.upk`

Required assertions:

- parser reads both inputs
- rebuilt output matches the expected known-answer contract
- validation confirms body equality and target identity behavior

## 2. Affluenter on Unzipped

Plan inputs:

- target `4916`
- source `7598`

Historical significance:

- validated in game with the Python method

Required assertions:

- target visual and thumbnail UPKs resolve correctly
- source visual and thumbnail UPKs resolve correctly
- build passes target-identity validation
- dry-run install preview shows exact affected files and backup destinations

## 3. Contagion on Standard Banner

Plan inputs:

- target `2526`
- source `2479`

Historical significance:

- validated banner build case

Required assertions:

- banner visual and thumbnail targets resolve correctly
- rebuilt outputs validate successfully
- dry-run preview shows exact banner file paths and backup paths

## 4. Cosmological on Bubbly

Plan inputs:

- target `1888`
- source `11330`

Required assertions:

- plan selects the correct Bubbly target package set
- source package set resolves to Cosmological
- rebuild validation passes with body equality intact

## 5. Lunation on 20XX

Plan inputs:

- target `1684`
- source `7856`

Required assertions:

- plan resolves correct 20XX target package set
- Lunation source package set resolves correctly
- validation confirms target identity and modified export refs

## 6. Laser Wave III on Gaseous

Plan inputs:

- target `10708`
- source `3224`

Required assertions:

- plan resolves correct Gaseous target package set
- Laser Wave III source package set resolves correctly
- dry-run preview includes exact backups and install targets

## Expected Test Harness Behavior

- tests create a temporary sandbox cooked root
- tests write plans and builds under a temporary workspace
- tests may also write builds under an explicit temporary sandbox output root
- tests may execute confirmation-gated install only against temporary sandbox cooked roots
- tests may execute confirmation-gated restore only against temporary sandbox cooked roots
- tests fail fast on any missing local file or validation mismatch
- planner-only tests may stop before build/install as long as blockers and warnings are explicit and correct

## Minimum Release Gate

The Rust rewrite is not at feature parity until all six golden cases pass in a sandbox and the preview-plus-confirmed install and restore paths remain explicit, sandbox-safe, and correct.