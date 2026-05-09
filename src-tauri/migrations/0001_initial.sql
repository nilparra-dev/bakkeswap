PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS products (
    product_id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    slot TEXT,
    slot_id INTEGER,
    quality TEXT,
    paintable INTEGER NOT NULL DEFAULT 0,
    visual_upk TEXT,
    thumb_upk TEXT,
    visual_asset TEXT,
    thumbnail_asset TEXT,
    product_asset_package TEXT,
    product_asset_path TEXT,
    product_thumbnail_package TEXT,
    product_thumbnail_asset TEXT,
    source_dump TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS slots (
    slot_id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    label TEXT,
    plural_label TEXT,
    description TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS paints (
    paint_id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    label TEXT NOT NULL,
    colors TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS titles (
    title_id TEXT PRIMARY KEY,
    title_text TEXT NOT NULL,
    category TEXT,
    color TEXT,
    glow_color TEXT,
    sort_priority INTEGER,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS local_files (
    path TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    kind TEXT NOT NULL,
    exists_on_disk INTEGER NOT NULL DEFAULT 1,
    size_bytes INTEGER,
    sha256 TEXT,
    cooked_root TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS swap_plans (
    plan_id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL,
    target_product_id INTEGER NOT NULL,
    source_product_id INTEGER NOT NULL,
    target_visual_upk TEXT,
    target_thumb_upk TEXT,
    source_visual_upk TEXT,
    source_thumb_upk TEXT,
    target_visual_identity TEXT,
    target_thumb_identity TEXT,
    build_method TEXT NOT NULL,
    plan_path TEXT,
    cooked_root TEXT,
    notes_json TEXT,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY (target_product_id) REFERENCES products(product_id),
    FOREIGN KEY (source_product_id) REFERENCES products(product_id)
);

CREATE TABLE IF NOT EXISTS builds (
    build_id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    build_root TEXT NOT NULL,
    visual_output_path TEXT,
    thumb_output_path TEXT,
    validation_json TEXT,
    body_matches_source INTEGER NOT NULL DEFAULT 0,
    target_identity_present INTEGER NOT NULL DEFAULT 0,
    modified_export_refs_detected INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY (plan_id) REFERENCES swap_plans(plan_id)
);

CREATE TABLE IF NOT EXISTS installed_swaps (
    install_id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    profile_name TEXT NOT NULL,
    cooked_root TEXT NOT NULL,
    manifest_path TEXT,
    installed_at TEXT NOT NULL,
    restored_at TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    dry_run_only INTEGER NOT NULL DEFAULT 0,
    files_json TEXT NOT NULL,
    FOREIGN KEY (plan_id) REFERENCES swap_plans(plan_id)
);

CREATE TABLE IF NOT EXISTS original_backups (
    backup_id TEXT PRIMARY KEY,
    target_relative_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    backup_kind TEXT NOT NULL,
    profile_name TEXT,
    cooked_root TEXT NOT NULL,
    verified_at TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (target_relative_path, backup_kind, profile_name)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_products_name ON products(name);
CREATE INDEX IF NOT EXISTS idx_products_slot ON products(slot);
CREATE INDEX IF NOT EXISTS idx_titles_text ON titles(title_text);
CREATE INDEX IF NOT EXISTS idx_local_files_filename ON local_files(filename);
CREATE INDEX IF NOT EXISTS idx_swap_plans_profile ON swap_plans(profile_name);
CREATE INDEX IF NOT EXISTS idx_builds_plan ON builds(plan_id);
CREATE INDEX IF NOT EXISTS idx_installed_swaps_profile ON installed_swaps(profile_name, active);
CREATE INDEX IF NOT EXISTS idx_original_backups_target ON original_backups(target_relative_path, backup_kind);