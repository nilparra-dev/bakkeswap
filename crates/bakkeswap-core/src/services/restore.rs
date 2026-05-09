use anyhow::{bail, Result};

#[derive(Debug, Default)]
pub struct RestoreService;

impl RestoreService {
    pub fn restore_profile(&self, _profile_name: &str) -> Result<()> {
        bail!("not implemented: restore service")
    }
}
