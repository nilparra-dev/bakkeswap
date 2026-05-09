# SQLite Schema Draft

## Goal

Replace the Python prototype's app-local JSON state with an explicit SQLite schema while preserving the same safety model and auditability.

Migration file:

- `src-tauri/migrations/0001_initial.sql`

## Design Principles

- keep imported metadata separate from local file discovery
- treat plans, builds, installs, and backups as auditable records
- support dry-run install previews without touching the real game files
- keep settings in the database, but do not hide important operator-visible file paths

## Tables

### `products`

Purpose:

- imported product metadata from CodeRed and merged local indexing

Core fields:

- `product_id`
- `name`
- `slot`
- `slot_id`
- `quality`
- `paintable`
- `visual_upk`
- `thumb_upk`
- `visual_asset`
- `thumbnail_asset`
- `source_dump`
- `updated_at`

### `slots`

Purpose:

- normalized slot metadata

Core fields:

- `slot_id`
- `name`
- `label`
- `plural_label`
- `description`
- `updated_at`

### `paints`

Purpose:

- normalized paint metadata

Core fields:

- `paint_id`
- `name`
- `label`
- `colors`
- `updated_at`

### `titles`

Purpose:

- title metadata from CodeRed dumps only
- v1 uses this for database completeness, not for a runtime title module

Core fields:

- `title_id`
- `title_text`
- `category`
- `color`
- `glow_color`
- `sort_priority`
- `updated_at`

### `local_files`

Purpose:

- current local CookedPCConsole file discovery state

Core fields:

- `path`
- `filename`
- `kind`
- `exists_on_disk`
- `size_bytes`
- `sha256`
- `cooked_root`
- `updated_at`

### `swap_plans`

Purpose:

- resolved TARGET and SOURCE relationships before build

Core fields:

- `plan_id`
- `profile_name`
- `target_product_id`
- `source_product_id`
- `target_visual_upk`
- `target_thumb_upk`
- `source_visual_upk`
- `source_thumb_upk`
- `target_visual_identity`
- `target_thumb_identity`
- `build_method`
- `plan_path`
- `cooked_root`
- `notes_json`
- `created_at`
- `status`

### `builds`

Purpose:

- build outputs and validation status for each plan

Core fields:

- `build_id`
- `plan_id`
- `build_root`
- `visual_output_path`
- `thumb_output_path`
- `validation_json`
- `body_matches_source`
- `target_identity_present`
- `modified_export_refs_detected`
- `created_at`
- `status`

### `installed_swaps`

Purpose:

- install manifest and active-state tracking

Core fields:

- `install_id`
- `plan_id`
- `profile_name`
- `cooked_root`
- `manifest_path`
- `installed_at`
- `restored_at`
- `active`
- `dry_run_only`
- `files_json`

### `original_backups`

Purpose:

- permanent original backup tracking
- profile-scoped backup tracking through `backup_kind` plus `profile_name`

Core fields:

- `backup_id`
- `target_relative_path`
- `backup_path`
- `sha256`
- `backup_kind`
- `profile_name`
- `cooked_root`
- `verified_at`
- `created_at`

### `settings`

Purpose:

- application configuration such as game path, selected database source, and UI preferences

Core fields:

- `key`
- `value_json`
- `updated_at`

## Status Tracking Strategy

Status tracking in v1 is derived from:

- `swap_plans.status`
- `builds.status`
- `installed_swaps.active`
- backup verification timestamps
- current validated game path from `settings`

No separate status table is required in the first draft.

## Schema Constraints

- `products`, `slots`, `paints`, `titles` are import-owned data
- `swap_plans`, `builds`, `installed_swaps`, and `original_backups` are operator and runtime-owned data
- plan and build records must remain queryable after restore for auditability
- automated dry-runs may create plan and preview records, but must not produce active install rows against the real game path

## Safety Implications

- a successful schema migration does not permit a real install
- real install still requires explicit operator action and a validated game path
- schema convenience must not hide exact file overwrite or backup details from the CLI or GUI