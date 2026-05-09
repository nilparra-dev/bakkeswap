use anyhow::{bail, Result};

#[derive(Debug, Default)]
pub struct TableCipher;

impl TableCipher {
    pub fn decrypt_tables(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        bail!("not implemented: table decryption")
    }

    pub fn encrypt_tables(&self, _bytes: &[u8]) -> Result<Vec<u8>> {
        bail!("not implemented: table encryption")
    }
}
