use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRedImportSource {
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseImportSummary {
    pub imported_products: usize,
    pub imported_slots: usize,
    pub imported_paints: usize,
    pub imported_titles: usize,
}

#[derive(Debug, Default)]
pub struct DatabaseImporter;

impl DatabaseImporter {
    pub fn import_codered(&self, _source: &CodeRedImportSource) -> Result<DatabaseImportSummary> {
        bail!("not implemented: CodeRed database import")
    }

    pub fn refresh(&self) -> Result<DatabaseImportSummary> {
        bail!("not implemented: database refresh")
    }
}
