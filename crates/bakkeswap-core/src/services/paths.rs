use anyhow::{bail, Result};

use crate::domain::models::GamePathValidation;

#[derive(Debug, Default)]
pub struct PathService;

impl PathService {
    pub fn validate_game_path(&self, _path: &str) -> Result<GamePathValidation> {
        bail!("not implemented: game path validation")
    }

    pub fn show_config(&self) -> Result<()> {
        bail!("not implemented: path config show")
    }
}
