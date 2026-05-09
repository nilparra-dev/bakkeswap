use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::format::{hash_bytes, PackageSummary, DEFAULT_TABLE_KEY};
use super::reader::ByteReader;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependsEntry {
    pub index: usize,
    pub value: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependsTable {
    pub entries: Vec<DependsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableDecryptionInfo {
    pub key_sha256: String,
    pub chunk_count_probe: i32,
    pub first_uncompressed_offset_probe: i32,
}

pub fn decrypt_table_region(
    raw: &[u8],
    summary: &PackageSummary,
) -> Result<(Vec<u8>, TableDecryptionInfo)> {
    let encrypted_size = summary.encrypted_region_size()?;
    let name_offset = summary.name_offset_usize()?;
    let end = name_offset
        .checked_add(encrypted_size)
        .ok_or_else(|| anyhow!("encrypted region end overflow"))?;
    let encrypted = raw
        .get(name_offset..end)
        .ok_or_else(|| anyhow!("failed to read encrypted table region"))?;

    let compressed_chunks_offset = summary.compressed_chunks_offset_usize()?;
    let block_offset = compressed_chunks_offset % 16;
    let block_start = compressed_chunks_offset
        .checked_sub(block_offset)
        .ok_or_else(|| anyhow!("compressed chunk block offset underflow"))?;
    let probe = encrypted
        .get(block_start..block_start + 32)
        .ok_or_else(|| anyhow!("failed to read encrypted probe for chunk metadata"))?;

    let plain_probe = decrypt_ecb(probe, &DEFAULT_TABLE_KEY)?;
    let mut probe_reader = ByteReader::with_offset(&plain_probe, block_offset)?;
    let chunk_count_probe = probe_reader.read_i32()?;
    let first_uncompressed_offset_probe = probe_reader.read_i32()?;
    if chunk_count_probe < 1 || first_uncompressed_offset_probe != summary.depends_offset {
        return Err(anyhow!(
            "default table key did not decrypt a valid compressed chunk table probe"
        ));
    }

    let decrypted = decrypt_ecb(encrypted, &DEFAULT_TABLE_KEY)?;
    Ok((
        decrypted,
        TableDecryptionInfo {
            key_sha256: hash_bytes(&DEFAULT_TABLE_KEY),
            chunk_count_probe,
            first_uncompressed_offset_probe,
        },
    ))
}

pub fn parse_depends_table(decrypted: &[u8], summary: &PackageSummary) -> Result<DependsTable> {
    let (start, end) = depends_region_bounds(summary)?;
    if end < start {
        return Err(anyhow!("depends table end is before its start"));
    }
    let size = end - start;
    if size == 0 {
        return Ok(DependsTable::default());
    }
    if size % 4 != 0 {
        return Err(anyhow!(
            "depends table region is not aligned to 4-byte entries"
        ));
    }

    let mut reader = ByteReader::with_offset(decrypted, start)?;
    let count = size / 4;
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        entries.push(DependsEntry {
            index,
            value: reader.read_i32()?,
        });
    }
    Ok(DependsTable { entries })
}

pub fn depends_region_bounds(summary: &PackageSummary) -> Result<(usize, usize)> {
    let start = summary.relative_to_name_offset(summary.depends_offset, "depends_offset")?;
    let end = determine_depends_end(summary, start)?;
    Ok((start, end))
}

pub fn encrypt_table_region(plain: &[u8]) -> Result<Vec<u8>> {
    encrypt_ecb(plain, &DEFAULT_TABLE_KEY)
}

fn determine_depends_end(summary: &PackageSummary, start: usize) -> Result<usize> {
    let mut candidates = vec![summary.compressed_chunks_offset_usize()?];

    for (offset, label) in [
        (
            summary.import_export_guid_offset,
            "import_export_guid_offset",
        ),
        (summary.thumbnail_table_offset, "thumbnail_table_offset"),
    ] {
        if offset > summary.depends_offset {
            candidates.push(summary.relative_to_name_offset(offset, label)?);
        }
    }

    candidates.retain(|candidate| *candidate >= start);
    candidates
        .into_iter()
        .min()
        .ok_or_else(|| anyhow!("could not determine depends table end"))
}

fn decrypt_ecb(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if !encrypted.len().is_multiple_of(16) {
        return Err(anyhow!(
            "AES-ECB region size must be a multiple of 16 bytes"
        ));
    }

    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut decrypted = encrypted.to_vec();
    for block in decrypted.chunks_exact_mut(16) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(decrypted)
}

fn encrypt_ecb(plain: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if !plain.len().is_multiple_of(16) {
        return Err(anyhow!(
            "AES-ECB region size must be a multiple of 16 bytes"
        ));
    }

    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut encrypted = plain.to_vec();
    for block in encrypted.chunks_exact_mut(16) {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(encrypted)
}

#[cfg(test)]
mod tests {
    use super::{decrypt_table_region, encrypt_table_region};
    use crate::upk::format::PackageSummary;

    #[test]
    fn encrypts_and_decrypts_table_region_roundtrip() {
        let mut plain = vec![0u8; 32];
        plain[0..4].copy_from_slice(&(1i32).to_le_bytes());
        plain[4..8].copy_from_slice(&(128i32).to_le_bytes());

        let encrypted = encrypt_table_region(&plain).unwrap();
        let mut raw = vec![0u8; 64];
        raw.extend_from_slice(&encrypted);
        let summary = PackageSummary {
            magic: 0x9E2A83C1,
            file_version: 0,
            licensee_version: 22,
            total_header_size: 96,
            folder_name: String::new(),
            package_flags: 0,
            name_count: 0,
            name_offset: 64,
            export_count: 0,
            export_offset: 64,
            import_count: 0,
            import_offset: 64,
            depends_offset: 128,
            import_export_guid_offset: 128,
            import_guid_count: 0,
            export_guid_count: 0,
            thumbnail_table_offset: 128,
            guid: String::new(),
            generations: Vec::new(),
            engine_version: 0,
            cooker_version: 0,
            compression_flags: 0,
            summary_compressed_chunks: Vec::new(),
            package_source: 0,
            additional_packages: Vec::new(),
            metadata_offset: 0,
            garbage_size: 0,
            compressed_chunks_offset: 0,
            last_block_size: 0,
        };

        let (roundtrip, _) = decrypt_table_region(&raw, &summary).unwrap();
        assert_eq!(roundtrip, plain);
    }
}
