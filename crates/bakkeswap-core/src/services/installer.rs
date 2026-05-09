use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPreview {
    pub plan_path: String,
    pub affected_files: Vec<String>,
    pub backup_files: Vec<String>,
    pub dry_run_only: bool,
}

#[derive(Debug, Default)]
pub struct InstallerService;

impl InstallerService {
    pub fn preview_install(&self, _plan_path: &str) -> Result<InstallPreview> {
        bail!("not implemented: installer preview")
    }

    pub fn install(&self, _plan_path: &str) -> Result<InstallPreview> {
        bail!("not implemented: installer execution")
    }
}
