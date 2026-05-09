use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::compression::{decompress_body, parse_rl_compressed_chunks};
use super::exports::ExportTable;
use super::format::{hash_bytes, PackageSummary};
use super::imports::ImportTable;
use super::names::NameTable;
use super::tables::{decrypt_table_region, parse_depends_table, TableDecryptionInfo};
use super::validate::{
    collect_string_evidence, collect_table_name_evidence, validate_summary, UpkInspectStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpkInspectReport {
    pub filename: String,
    pub path: String,
    pub file_size: u64,
    pub sha256: String,
    pub magic: String,
    pub file_version: u16,
    pub licensee_version: u16,
    pub is_probable_rocket_league: bool,
    pub total_header_size: i32,
    pub package_flags: String,
    pub compression_flags: String,
    pub name_count: usize,
    pub import_count: usize,
    pub export_count: usize,
    pub depends_count: Option<usize>,
    pub compressed_chunk_count: Option<usize>,
    pub decompressed_body_size: Option<usize>,
    pub decompressed_body_sha256: Option<String>,
    pub decryption: Option<TableDecryptionInfo>,
    pub status: UpkInspectStatus,
    pub string_evidence: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct UpkInspector;

impl UpkInspector {
    pub fn inspect_path(&self, path: &Path) -> Result<UpkInspectReport> {
        let raw = fs::read(path)
            .with_context(|| format!("failed to read UPK from {}", path.display()))?;
        let summary = PackageSummary::parse(&raw)?;

        let mut report = UpkInspectReport {
            filename: path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
            path: path.display().to_string(),
            file_size: raw.len() as u64,
            sha256: hash_bytes(&raw),
            magic: format!("0x{:08X}", summary.magic),
            file_version: summary.file_version,
            licensee_version: summary.licensee_version,
            is_probable_rocket_league: summary.is_probable_rocket_league(),
            total_header_size: summary.total_header_size,
            package_flags: format!("0x{:08X}", summary.package_flags),
            compression_flags: format!("0x{:X}", summary.compression_flags),
            name_count: summary.name_count,
            import_count: summary.import_count,
            export_count: summary.export_count,
            depends_count: None,
            compressed_chunk_count: None,
            decompressed_body_size: None,
            decompressed_body_sha256: None,
            decryption: None,
            status: UpkInspectStatus {
                summary_parsed: true,
                detected_rocket_league_format: summary.is_probable_rocket_league(),
                ..UpkInspectStatus::default()
            },
            string_evidence: Vec::new(),
            warnings: validate_summary(&summary),
        };

        match decrypt_table_region(&raw, &summary) {
            Ok((decrypted, info)) => {
                report.status.tables_decrypted = true;
                report.decryption = Some(info);

                let mut name_table: Option<NameTable> = None;
                match NameTable::parse(&decrypted, &summary) {
                    Ok(parsed) => {
                        report.status.name_table_parsed = true;
                        report.string_evidence = collect_table_name_evidence(&parsed, 20);
                        name_table = Some(parsed);
                    }
                    Err(error) => report
                        .warnings
                        .push(format!("NameTable parse failed: {error}")),
                }

                if let Some(name_table) = &name_table {
                    match ImportTable::parse(&decrypted, &summary, name_table) {
                        Ok(_) => report.status.import_table_parsed = true,
                        Err(error) => report
                            .warnings
                            .push(format!("ImportTable parse failed: {error}")),
                    }
                    match ExportTable::parse(&decrypted, &summary, name_table) {
                        Ok(_) => report.status.export_table_parsed = true,
                        Err(error) => report
                            .warnings
                            .push(format!("ExportTable parse failed: {error}")),
                    }
                }

                match parse_depends_table(&decrypted, &summary) {
                    Ok(depends) => {
                        report.status.depends_table_parsed = true;
                        report.depends_count = Some(depends.entries.len());
                    }
                    Err(error) => report
                        .warnings
                        .push(format!("DependsTable parse failed: {error}")),
                }

                match parse_rl_compressed_chunks(
                    &decrypted,
                    summary.compressed_chunks_offset_usize()?,
                ) {
                    Ok(chunks) => {
                        report.status.compressed_chunks_parsed = true;
                        report.compressed_chunk_count = Some(chunks.len());
                        match decompress_body(&raw, &chunks) {
                            Ok(body) => {
                                report.status.body_decompressed = true;
                                report.decompressed_body_size = Some(body.len());
                                report.decompressed_body_sha256 = Some(hash_bytes(&body));
                                if report.string_evidence.is_empty() {
                                    report.string_evidence = collect_string_evidence(&body, 20);
                                }
                            }
                            Err(error) => report
                                .warnings
                                .push(format!("body decompression failed: {error}")),
                        }
                    }
                    Err(error) => report
                        .warnings
                        .push(format!("compressed chunk metadata parse failed: {error}")),
                }
            }
            Err(error) => report
                .warnings
                .push(format!("table decryption failed: {error}")),
        }

        if report.string_evidence.is_empty() {
            report.string_evidence = collect_string_evidence(&raw, 20);
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
    use aes::Aes256;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use tempfile::TempDir;

    use super::UpkInspector;
    use crate::upk::format::{DEFAULT_TABLE_KEY, PACKAGE_TAG, RL_COMPRESSED_CHUNK_MAGIC};

    #[test]
    fn inspects_synthetic_package() {
        let temp = TempDir::new().unwrap();
        let package_path = temp.path().join("synthetic_read_only.upk");
        let expected_body = b"synthetic rl upk body";
        std::fs::write(&package_path, build_synthetic_package(expected_body)).unwrap();

        let report = UpkInspector.inspect_path(&package_path).unwrap();
        assert_eq!(report.filename, "synthetic_read_only.upk");
        assert!(report.is_probable_rocket_league);
        assert_eq!(report.name_count, 1);
        assert_eq!(report.import_count, 0);
        assert_eq!(report.export_count, 0);
        assert_eq!(report.depends_count, Some(0));
        assert_eq!(report.compressed_chunk_count, Some(1));
        assert_eq!(report.decompressed_body_size, Some(expected_body.len()));
        assert!(report.status.tables_decrypted);
        assert!(report.status.name_table_parsed);
        assert!(report.status.compressed_chunks_parsed);
        assert!(report.status.body_decompressed);
        assert!(report
            .string_evidence
            .iter()
            .any(|value| value.contains("TestName")));
    }

    fn build_synthetic_package(body: &[u8]) -> Vec<u8> {
        let name_offset = synthetic_summary_size() as i32;
        let name_table = make_name_table("TestName");
        let compressed_chunk_bytes = make_compressed_chunk(body);
        let compressed_chunks_offset = i32::try_from(name_table.len()).unwrap();
        let import_offset = name_offset + compressed_chunks_offset;
        let depends_offset = import_offset;
        let encrypted_region_size = align16(name_table.len() + rl_chunk_table_size());
        let total_header_size = name_offset as usize + encrypted_region_size;
        let rl_chunk_table = make_rl_chunk_table(
            depends_offset,
            compressed_chunk_bytes.len() as i32,
            total_header_size as i64,
            body.len() as i32,
        );
        let encrypted_region = encrypt_region(&build_region(
            &name_table,
            &rl_chunk_table,
            encrypted_region_size,
        ));
        let summary = build_summary(
            name_offset,
            total_header_size as i32,
            import_offset,
            depends_offset,
            compressed_chunks_offset,
        );

        let mut raw = summary;
        raw.extend_from_slice(&encrypted_region);
        raw.extend_from_slice(&compressed_chunk_bytes);
        raw
    }

    fn build_summary(
        name_offset: i32,
        total_header_size: i32,
        import_offset: i32,
        depends_offset: i32,
        compressed_chunks_offset: i32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PACKAGE_TAG.to_le_bytes());
        bytes.extend_from_slice(&845u16.to_le_bytes());
        bytes.extend_from_slice(&22u16.to_le_bytes());
        bytes.extend_from_slice(&total_header_size.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(1i32).to_le_bytes());
        bytes.extend_from_slice(&name_offset.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&import_offset.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&import_offset.to_le_bytes());
        bytes.extend_from_slice(&depends_offset.to_le_bytes());
        bytes.extend_from_slice(&import_offset.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&import_offset.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&compressed_chunks_offset.to_le_bytes());
        bytes.extend_from_slice(&(body_block_size() as i32).to_le_bytes());
        bytes
    }

    fn synthetic_summary_size() -> usize {
        build_summary(0, 0, 0, 0, 0).len()
    }

    fn build_region(name_table: &[u8], rl_chunk_table: &[u8], total_size: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(name_table);
        bytes.extend_from_slice(rl_chunk_table);
        bytes.resize(total_size, 0);
        bytes
    }

    fn make_name_table(name: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((name.len() as i32) + 1).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes
    }

    fn make_rl_chunk_table(
        uncompressed_offset: i32,
        compressed_size: i32,
        compressed_offset: i64,
        uncompressed_size: i32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(1i32).to_le_bytes());
        bytes.extend_from_slice(&(i64::from(uncompressed_offset)).to_le_bytes());
        bytes.extend_from_slice(&uncompressed_size.to_le_bytes());
        bytes.extend_from_slice(&compressed_offset.to_le_bytes());
        bytes.extend_from_slice(&compressed_size.to_le_bytes());
        bytes
    }

    fn rl_chunk_table_size() -> usize {
        make_rl_chunk_table(0, 0, 0, 0).len()
    }

    fn make_compressed_chunk(body: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&RL_COMPRESSED_CHUNK_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&(body_block_size() as u32).to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        bytes
    }

    fn encrypt_region(plain: &[u8]) -> Vec<u8> {
        let cipher = Aes256::new(GenericArray::from_slice(&DEFAULT_TABLE_KEY));
        let mut encrypted = plain.to_vec();
        for block in encrypted.chunks_exact_mut(16) {
            cipher.encrypt_block(GenericArray::from_mut_slice(block));
        }
        encrypted
    }

    fn align16(value: usize) -> usize {
        (value + 15) & !15
    }

    fn body_block_size() -> usize {
        0x20000
    }
}
