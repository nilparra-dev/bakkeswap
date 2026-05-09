pub const REQUIRED_TABLES: &[&str] = &[
    "products",
    "slots",
    "paints",
    "titles",
    "local_files",
    "swap_plans",
    "builds",
    "installed_swaps",
    "original_backups",
    "settings",
];

pub const fn current_schema_version() -> i64 {
    1
}
