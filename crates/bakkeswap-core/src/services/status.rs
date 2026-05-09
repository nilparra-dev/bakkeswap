use anyhow::Result;
use rusqlite::params;
use rusqlite::OptionalExtension;

use crate::database::DatabaseService;
use crate::domain::models::AppStatus;

const GAME_PATH_INPUT_KEY: &str = "game_path_input";
const COOKED_DIR_KEY: &str = "cooked_dir";
const CODERED_DUMPS_DIR_KEY: &str = "codered_dumps_dir";

#[derive(Debug, Clone)]
pub struct StatusService {
    database: DatabaseService,
}

impl StatusService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn current_status(&self) -> Result<AppStatus> {
        let connection = self.database.connect()?;
        let product_count = count_query(&connection, "SELECT COUNT(*) FROM products")? as usize;
        let title_count = count_query(&connection, "SELECT COUNT(*) FROM titles")? as usize;
        let local_files_count =
            count_query(&connection, "SELECT COUNT(*) FROM local_files")? as usize;
        let active_swap_count = count_query(
            &connection,
            "SELECT COUNT(*) FROM installed_swaps WHERE active = 1 AND dry_run_only = 0",
        )? as usize;
        let original_backup_count = count_query(
            &connection,
            "SELECT COUNT(*) FROM original_backups WHERE backup_kind = 'original'",
        )? as usize;
        let profile_backup_count = count_query(
            &connection,
            "SELECT COUNT(*) FROM original_backups WHERE backup_kind = 'profile'",
        )? as usize;

        Ok(AppStatus {
            configured_game_path: setting(&connection, GAME_PATH_INPUT_KEY)?,
            configured_cooked_dir: setting(&connection, COOKED_DIR_KEY)?,
            configured_codered_dumps_dir: setting(&connection, CODERED_DUMPS_DIR_KEY)?,
            database_ready: true,
            local_files_indexed: local_files_count > 0,
            product_count,
            title_count,
            active_swap_count,
            original_backup_count,
            profile_backup_count,
        })
    }
}

fn count_query(connection: &rusqlite::Connection, sql: &str) -> Result<i64> {
    Ok(connection.query_row(sql, [], |row| row.get(0))?)
}

fn setting(connection: &rusqlite::Connection, key: &str) -> Result<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|value| serde_json::from_str::<String>(&value))
        .transpose()?)
}
