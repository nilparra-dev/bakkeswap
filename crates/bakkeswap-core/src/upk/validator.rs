use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpkValidationReport {
    pub body_matches_source: bool,
    pub target_identity_present: bool,
    pub modified_export_refs_detected: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub struct UpkValidator;

impl UpkValidator {
    pub fn validate_rebuild(
        &self,
        _built_visual_path: &str,
        _source_visual_path: &str,
    ) -> Result<UpkValidationReport> {
        bail!("not implemented: upk validator")
    }
}
