use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildRequest {
    pub source_visual_upk: String,
    pub target_visual_upk: String,
    pub source_thumb_upk: Option<String>,
    pub target_thumb_upk: Option<String>,
    pub target_visual_identity: String,
    pub target_thumb_identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildResult {
    pub visual_output_path: String,
    pub thumb_output_path: Option<String>,
    pub source_body_matches_output_body: bool,
    pub target_identity_present: bool,
    pub modified_export_refs_detected: bool,
}

#[derive(Debug, Default)]
pub struct TargetIdentityRebuilder;

impl TargetIdentityRebuilder {
    pub fn rebuild(&self, _request: &RebuildRequest) -> Result<RebuildResult> {
        bail!("not implemented: target identity rebuilder")
    }
}
