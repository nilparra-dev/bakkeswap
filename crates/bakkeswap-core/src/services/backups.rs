use anyhow::{bail, Result};

#[derive(Debug, Default)]
pub struct PermanentOriginalBackupManager;

impl PermanentOriginalBackupManager {
    pub fn status(&self) -> Result<()> {
        bail!("not implemented: permanent original backup status")
    }

    pub fn verify(&self) -> Result<()> {
        bail!("not implemented: permanent original backup verify")
    }
}

#[derive(Debug, Default)]
pub struct ProfileBackupManager;

impl ProfileBackupManager {
    pub fn restore_profile(&self, _profile_name: &str) -> Result<()> {
        bail!("not implemented: profile backup restore")
    }
}
