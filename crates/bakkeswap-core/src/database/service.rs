use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

const INITIAL_MIGRATION_SQL: &str =
    include_str!("../../../../src-tauri/migrations/0001_initial.sql");
const APP_HOME_ENV: &str = "BAKKESWAP_APP_HOME";
const DATABASE_FILENAME: &str = "bakkeswap.sqlite3";

#[derive(Debug, Clone)]
pub struct DatabaseService {
    app_home: PathBuf,
    database_path: PathBuf,
}

impl DatabaseService {
    pub fn for_current_user() -> Result<Self> {
        let app_home = resolve_default_app_home()?;
        Ok(Self::from_app_home(app_home))
    }

    pub fn from_app_home(app_home: impl AsRef<Path>) -> Self {
        let app_home = app_home.as_ref().to_path_buf();
        let database_path = app_home.join(DATABASE_FILENAME);
        Self {
            app_home,
            database_path,
        }
    }

    pub fn app_home(&self) -> &Path {
        &self.app_home
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn connect(&self) -> Result<Connection> {
        fs::create_dir_all(&self.app_home).with_context(|| {
            format!(
                "failed to create BakkesSwap app data directory at {}",
                self.app_home.display()
            )
        })?;

        let connection = Connection::open(&self.database_path).with_context(|| {
            format!(
                "failed to open BakkesSwap database at {}",
                self.database_path.display()
            )
        })?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        self.run_migrations(&connection)?;
        Ok(connection)
    }

    pub fn set_string_setting(&self, key: &str, value: &str) -> Result<()> {
        self.set_json_setting(key, value)
    }

    pub fn get_string_setting(&self, key: &str) -> Result<Option<String>> {
        self.get_json_setting(key)
    }

    pub fn set_json_setting<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let connection = self.connect()?;
        let value_json = serde_json::to_string(value)?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, value_json, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_json_setting<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let connection = self.connect()?;
        let raw_value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        raw_value
            .map(|value| {
                serde_json::from_str(&value).context("failed to decode stored JSON setting")
            })
            .transpose()
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        let connection = self.connect()?;
        connection.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn count_rows(&self, table: &str) -> Result<i64> {
        let connection = self.connect()?;
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count = connection.query_row(&sql, [], |row| row.get(0))?;
        Ok(count)
    }

    fn run_migrations(&self, connection: &Connection) -> Result<()> {
        connection.execute_batch(INITIAL_MIGRATION_SQL)?;
        Ok(())
    }
}

fn resolve_default_app_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os(APP_HOME_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(local_app_data) = env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(local_app_data).join("BakkesSwap"));
    }

    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("BakkesSwap"));
    }

    env::current_dir()
        .map(|cwd| cwd.join(".bakkeswap"))
        .context("failed to determine a default BakkesSwap app data directory")
}
