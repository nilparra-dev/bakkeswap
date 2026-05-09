use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::DatabaseService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFileIndexSummary {
    pub cooked_root: String,
    pub indexed_files: usize,
}

#[derive(Debug, Clone)]
pub struct LocalFileIndexer {
    database: DatabaseService,
}

impl LocalFileIndexer {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn index_cooked_dir(&self, cooked_dir: &Path) -> Result<LocalFileIndexSummary> {
        if !cooked_dir.exists() {
            return Err(anyhow!(
                "configured CookedPCConsole path does not exist: {}",
                cooked_dir.display()
            ));
        }
        if !cooked_dir.is_dir() {
            return Err(anyhow!(
                "configured CookedPCConsole path is not a directory: {}",
                cooked_dir.display()
            ));
        }

        let cooked_root = cooked_dir
            .canonicalize()
            .unwrap_or_else(|_| cooked_dir.to_path_buf());
        let cooked_root_text = cooked_root.display().to_string();
        let mut entries = fs::read_dir(cooked_dir)
            .with_context(|| {
                format!(
                    "failed to read CookedPCConsole directory at {}",
                    cooked_dir.display()
                )
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());

        let mut connection = self.database.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM local_files WHERE cooked_root = ?1",
            params![cooked_root_text],
        )?;

        let mut indexed_files = 0usize;
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(extension) = path.extension() else {
                continue;
            };
            if !extension.to_string_lossy().eq_ignore_ascii_case("upk") {
                continue;
            }

            let metadata = entry.metadata()?;
            let filename = entry.file_name().to_string_lossy().to_string();
            let sha256 = sha256_path(&path)?;
            transaction.execute(
                "INSERT INTO local_files (path, filename, kind, exists_on_disk, size_bytes, sha256, cooked_root, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    path.display().to_string(),
                    filename,
                    "upk",
                    1,
                    metadata.len() as i64,
                    sha256,
                    cooked_root_text,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            indexed_files += 1;
        }

        transaction.commit()?;

        Ok(LocalFileIndexSummary {
            cooked_root: cooked_root_text,
            indexed_files,
        })
    }
}

fn sha256_path(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to open file for hashing: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
