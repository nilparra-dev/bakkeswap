use anyhow::{bail, Result};

use crate::domain::models::AppStatus;

#[derive(Debug, Default)]
pub struct StatusService;

impl StatusService {
    pub fn current_status(&self) -> Result<AppStatus> {
        bail!("not implemented: status service")
    }
}
