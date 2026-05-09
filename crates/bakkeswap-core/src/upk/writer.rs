use std::path::Path;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use super::rebuild::{
    resolve_output_filename, resolve_rebuild_profile_name, resolve_sandbox_output_path,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxWriteRequest {
    pub output_root: String,
    pub source_filename: String,
    pub target_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxWritePlan {
    pub profile_name: String,
    pub output_filename: String,
    pub output_path: String,
    pub sandbox_only: bool,
}

#[derive(Debug, Default, Clone)]
pub struct UpkWriter;

impl UpkWriter {
    pub fn plan_sandbox_write(&self, request: &SandboxWriteRequest) -> Result<SandboxWritePlan> {
        let profile_name =
            resolve_rebuild_profile_name(&request.source_filename, &request.target_filename)
                .ok_or_else(|| anyhow!("failed to derive rebuild profile name"))?;
        let output_filename = resolve_output_filename(&request.target_filename)
            .ok_or_else(|| anyhow!("failed to derive output filename"))?;
        let output_path = resolve_sandbox_output_path(
            Path::new(&request.output_root),
            &profile_name,
            &request.target_filename,
        )
        .ok_or_else(|| anyhow!("failed to derive sandbox output path"))?;

        Ok(SandboxWritePlan {
            profile_name,
            output_filename,
            output_path: output_path.display().to_string(),
            sandbox_only: true,
        })
    }

    pub fn write_sandbox_output(&self, _request: &SandboxWriteRequest) -> Result<SandboxWritePlan> {
        bail!("not implemented: sandbox-only UPK writer is not enabled yet")
    }
}
