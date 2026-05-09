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
- tests use dry-run install only
- tests fail fast on any missing local file or validation mismatch
- planner-only tests may stop before build/install as long as blockers and warnings are explicit and correct

## Minimum Release Gate

The Rust rewrite is not at feature parity until all six golden cases pass in a sandbox and the dry-run install preview remains explicit and correct.