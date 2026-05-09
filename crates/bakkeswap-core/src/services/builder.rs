use anyhow::{bail, Result};

use crate::domain::models::BuildRecord;

#[derive(Debug, Default)]
pub struct BuildService;

impl BuildService {
    pub fn build_plan(&self, _plan_path: &str) -> Result<BuildRecord> {
        bail!("not implemented: build service")
    }
}
