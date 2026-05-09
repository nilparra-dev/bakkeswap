use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::database::DatabaseService;
use crate::domain::models::GamePathValidation;

const GAME_PATH_INPUT_KEY: &str = "game_path_input";
const COOKED_DIR_KEY: &str = "cooked_dir";
const CODERED_DUMPS_DIR_KEY: &str = "codered_dumps_dir";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub game_path_input: Option<String>,
    pub cooked_dir: Option<String>,
    pub codered_dumps_dir: Option<String>,
    pub app_home: String,
    pub database_path: String,
    pub validation: Option<GamePathValidation>,
}

#[derive(Debug, Clone)]
pub struct PathService {
    database: DatabaseService,
}

impl PathService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn set_game_path(&self, path: &str) -> Result<GamePathValidation> {
        let validation = self.validate_game_path(path)?;
        if !validation.is_valid {
            return Err(anyhow!(format_validation_errors(&validation)));
        }

        self.database
            .set_string_setting(GAME_PATH_INPUT_KEY, path)?;
        if let Some(normalized) = &validation.normalized_cooked_dir {
            self.database
                .set_string_setting(COOKED_DIR_KEY, normalized)?;
        }

        Ok(validation)
    }

    pub fn validate_game_path(&self, path: &str) -> Result<GamePathValidation> {
        Ok(validate_game_path_value(path))
    }

    pub fn validate_configured_game_path(&self) -> Result<GamePathValidation> {
        let configured_input = self
            .database
            .get_string_setting(GAME_PATH_INPUT_KEY)?
            .or_else(|| self.database.get_string_setting(COOKED_DIR_KEY).ok().flatten())
            .ok_or_else(|| {
                anyhow!(
                    "no game path is configured yet; run 'bakkeswap config set-game-path <path>' first"
                )
            })?;
        Ok(validate_game_path_value(&configured_input))
    }

    pub fn configured_cooked_dir(&self) -> Result<Option<PathBuf>> {
        self.database
            .get_string_setting(COOKED_DIR_KEY)
            .map(|value| value.map(PathBuf::from))
    }

    pub fn show_config(&self) -> Result<ConfigSnapshot> {
        let game_path_input = self.database.get_string_setting(GAME_PATH_INPUT_KEY)?;
        let cooked_dir = self.database.get_string_setting(COOKED_DIR_KEY)?;
        let codered_dumps_dir = self.database.get_string_setting(CODERED_DUMPS_DIR_KEY)?;
        let validation = game_path_input
            .as_deref()
            .or(cooked_dir.as_deref())
            .map(validate_game_path_value);

        Ok(ConfigSnapshot {
            game_path_input,
            cooked_dir,
            codered_dumps_dir,
            app_home: self.database.app_home().display().to_string(),
            database_path: self.database.database_path().display().to_string(),
            validation,
        })
    }
}

pub fn validate_game_path_value(path: &str) -> GamePathValidation {
    let input_path = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return GamePathValidation {
            input_path: String::new(),
            normalized_cooked_dir: None,
            input_kind: None,
            is_valid: false,
            input_exists: false,
            cooked_exists: false,
            upk_count: 0,
            sample_upks: Vec::new(),
            warnings: Vec::new(),
            errors: vec!["No game path is configured. Expected a Rocket League root, TAGame, or CookedPCConsole path.".to_string()],
        };
    }

    let (normalized_cooked_dir, input_kind) = normalize_game_path_input(&input_path);
    let input_exists = input_path.exists();
    let cooked_exists = normalized_cooked_dir.exists();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    if !input_exists {
        errors.push(format!(
            "Selected path does not exist: {}",
            input_path.display()
        ));
    } else if !input_path.is_dir() {
        errors.push(format!(
            "Selected path is not a directory: {}",
            input_path.display()
        ));
    }

    if !cooked_exists {
        errors.push(format!(
            "Could not find CookedPCConsole under the selected {} path. Expected: {}",
            input_kind.as_deref().unwrap_or("game"),
            normalized_cooked_dir.display()
        ));
    } else if !normalized_cooked_dir.is_dir() {
        errors.push(format!(
            "CookedPCConsole path is not a directory: {}",
            normalized_cooked_dir.display()
        ));
    }

    let (upk_count, sample_upks) = if cooked_exists && normalized_cooked_dir.is_dir() {
        scan_upk_files(&normalized_cooked_dir)
    } else {
        (0, Vec::new())
    };

    if cooked_exists && normalized_cooked_dir.is_dir() {
        if upk_count == 0 {
            errors.push(format!(
                "No .upk files were found in {}",
                normalized_cooked_dir.display()
            ));
        } else if upk_count < 100 {
            warnings.push(format!(
                "Only {upk_count} .upk files were found in {}. A full Rocket League install usually contains many more package files.",
                normalized_cooked_dir.display()
            ));
        }
    }

    if input_path != normalized_cooked_dir {
        warnings.push(format!(
            "The selected {} path will be normalized to: {}",
            input_kind.as_deref().unwrap_or("game"),
            normalized_cooked_dir.display()
        ));
    }

    GamePathValidation {
        input_path: input_path.display().to_string(),
        normalized_cooked_dir: Some(normalized_cooked_dir.display().to_string()),
        input_kind,
        is_valid: errors.is_empty(),
        input_exists,
        cooked_exists,
        upk_count,
        sample_upks,
        warnings,
        errors,
    }
}

pub fn format_validation_errors(validation: &GamePathValidation) -> String {
    if validation.errors.is_empty() {
        return "configured game path is valid".to_string();
    }
    validation.errors.join(" | ")
}

pub fn normalize_game_path_input(path: &Path) -> (PathBuf, Option<String>) {
    let normalized_input = canonicalize_usable_path(path);
    let name = normalized_input
        .file_name()
        .map(|part| part.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if name == "cookedpcconsole" {
        return (normalized_input, Some("CookedPCConsole".to_string()));
    }
    if name == "tagame" {
        return (
            normalized_input.join("CookedPCConsole"),
            Some("TAGame".to_string()),
        );
    }

    let direct_cooked = normalized_input.join("CookedPCConsole");
    if direct_cooked.exists() {
        return (direct_cooked, Some("TAGame".to_string()));
    }

    (
        normalized_input.join("TAGame").join("CookedPCConsole"),
        Some("Rocket League root".to_string()),
    )
}

fn canonicalize_usable_path(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    strip_windows_verbatim_prefix(&canonical)
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.display().to_string();

    if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{stripped}"));
    }
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(stripped);
    }

    path.to_path_buf()
}

#[cfg(not(windows))]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn scan_upk_files(cooked_dir: &Path) -> (usize, Vec<String>) {
    let mut sample_upks = Vec::new();
    let mut count = 0usize;

    if let Ok(entries) = fs::read_dir(cooked_dir) {
        let mut names = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension()
                    .map(|extension| extension.to_string_lossy().eq_ignore_ascii_case("upk"))
                    .unwrap_or(false)
            })
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .collect::<Vec<_>>();
        names.sort_by_key(|name| name.to_ascii_lowercase());
        count = names.len();
        sample_upks.extend(names.into_iter().take(5));
    }

    (count, sample_upks)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{normalize_game_path_input, validate_game_path_value};

    #[test]
    fn normalizes_root_tagame_and_cooked_paths() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("RocketLeague");
        let tagame = root.join("TAGame");
        let cooked = tagame.join("CookedPCConsole");
        fs::create_dir_all(&cooked).unwrap();
        fs::write(cooked.join("Example_SF.upk"), b"fixture").unwrap();

        let (from_root, root_kind) = normalize_game_path_input(&root);
        assert_eq!(from_root, cooked);
        assert_eq!(root_kind.as_deref(), Some("Rocket League root"));

        let (from_tagame, tagame_kind) = normalize_game_path_input(&tagame);
        assert_eq!(from_tagame, cooked);
        assert_eq!(tagame_kind.as_deref(), Some("TAGame"));

        let (from_cooked, cooked_kind) = normalize_game_path_input(&cooked);
        assert_eq!(from_cooked, cooked);
        assert_eq!(cooked_kind.as_deref(), Some("CookedPCConsole"));
    }

    #[test]
    fn invalid_path_reports_helpful_error() {
        let validation = validate_game_path_value("Z:\\definitely\\missing\\rocketleague");
        assert!(!validation.is_valid);
        assert!(validation
            .errors
            .iter()
            .any(|message| message.contains("does not exist")));
    }
}
