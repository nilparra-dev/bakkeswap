use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::reader::ByteReader;

pub const PACKAGE_TAG: u32 = 0x9E2A83C1;
pub const RL_COMPRESSED_CHUNK_MAGIC: u32 = PACKAGE_TAG;
pub const DEFAULT_TABLE_KEY: [u8; 32] = [
    0xC7, 0xDF, 0x6B, 0x13, 0x25, 0x2A, 0xCC, 0x71, 0x47, 0xBB, 0x51, 0xC9, 0x8A, 0xD7, 0xE3, 0x4B,
    0x7F, 0xE5, 0x00, 0xB7, 0x7F, 0xA5, 0xFA, 0xB2, 0x93, 0xE2, 0xF2, 0x4E, 0x6B, 0x17, 0xE7, 0x79,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SummaryCompressedChunk {
    pub uncompressed_offset: i32,
    pub uncompressed_size: i32,
    pub compressed_offset: i32,
    pub compressed_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RocketLeagueCompressedChunk {
    pub uncompressed_offset: i64,
    pub uncompressed_size: i32,
    pub compressed_offset: i64,
    pub compressed_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationInfo {
    pub export_count: i32,
    pub name_count: i32,
    pub net_object_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameReference {
    pub name_index: i32,
    pub instance_number: i32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSummary {
    pub magic: u32,
    pub file_version: u16,
    pub licensee_version: u16,
    pub total_header_size: i32,
    pub folder_name: String,
    pub package_flags: u32,
    pub name_count: usize,
    pub name_offset: i32,
    pub export_count: usize,
    pub export_offset: i32,
    pub import_count: usize,
    pub import_offset: i32,
    pub depends_offset: i32,
    pub import_export_guid_offset: i32,
    pub import_guid_count: usize,
    pub export_guid_count: usize,
    pub thumbnail_table_offset: i32,
    pub guid: String,
    pub generations: Vec<GenerationInfo>,
    pub engine_version: u32,
    pub cooker_version: u32,
    pub compression_flags: u32,
    pub summary_compressed_chunks: Vec<SummaryCompressedChunk>,
    pub package_source: u32,
    pub additional_packages: Vec<String>,
    pub metadata_offset: i32,
    pub garbage_size: i32,
    pub compressed_chunks_offset: i32,
    pub last_block_size: i32,
}

impl PackageSummary {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let mut reader = ByteReader::new(raw);
        let magic = reader.read_u32()?;
        if magic != PACKAGE_TAG {
            return Err(anyhow!("bad UPK tag: 0x{magic:08X}"));
        }

        let file_version = reader.read_u16()?;
        let licensee_version = reader.read_u16()?;
        let total_header_size = reader.read_i32()?;
        let folder_name = reader.read_fstring()?;
        let package_flags = reader.read_u32()?;
        let name_count = reader.read_tarray_count()?;
        let name_offset = reader.read_i32()?;
        let export_count = reader.read_tarray_count()?;
        let export_offset = reader.read_i32()?;
        let import_count = reader.read_tarray_count()?;
        let import_offset = reader.read_i32()?;
        let depends_offset = reader.read_i32()?;
        let import_export_guid_offset = reader.read_i32()?;
        let import_guid_count = reader.read_tarray_count()?;
        let export_guid_count = reader.read_tarray_count()?;
        let thumbnail_table_offset = reader.read_i32()?;
        let guid = bytes_to_hex(reader.read_bytes(16)?);
        let generations_count = reader.read_tarray_count()?;
        let mut generations = Vec::with_capacity(generations_count);
        for _ in 0..generations_count {
            generations.push(GenerationInfo {
                export_count: reader.read_i32()?,
                name_count: reader.read_i32()?,
                net_object_count: reader.read_i32()?,
            });
        }
        let engine_version = reader.read_u32()?;
        let cooker_version = reader.read_u32()?;
        let compression_flags = reader.read_u32()?;
        let summary_chunk_count = reader.read_tarray_count()?;
        let mut summary_compressed_chunks = Vec::with_capacity(summary_chunk_count);
        for _ in 0..summary_chunk_count {
            summary_compressed_chunks.push(SummaryCompressedChunk {
                uncompressed_offset: reader.read_i32()?,
                uncompressed_size: reader.read_i32()?,
                compressed_offset: reader.read_i32()?,
                compressed_size: reader.read_i32()?,
            });
        }
        let package_source = reader.read_u32()?;
        let additional_packages_count = reader.read_tarray_count()?;
        let mut additional_packages = Vec::with_capacity(additional_packages_count);
        for _ in 0..additional_packages_count {
            additional_packages.push(reader.read_fstring()?);
        }
        let texture_allocations_count = reader.read_tarray_count()?;
        for _ in 0..texture_allocations_count {
            let _size_x = reader.read_i32()?;
            let _size_y = reader.read_i32()?;
            let _num_mips = reader.read_i32()?;
            let _format = reader.read_i32()?;
            let _tex_create_flags = reader.read_i32()?;
            let export_indices_count = reader.read_tarray_count()?;
            let skip_bytes = export_indices_count
                .checked_mul(4)
                .ok_or_else(|| anyhow!("texture allocation size overflow"))?;
            let next = reader
                .position()
                .checked_add(skip_bytes)
                .ok_or_else(|| anyhow!("texture allocation offset overflow"))?;
            reader.seek(next)?;
        }
        let metadata_offset =
            i32::try_from(reader.position()).map_err(|_| anyhow!("metadata offset overflow"))?;
        let garbage_size = reader.read_i32()?;
        let compressed_chunks_offset = reader.read_i32()?;
        let last_block_size = reader.read_i32()?;

        Ok(Self {
            magic,
            file_version,
            licensee_version,
            total_header_size,
            folder_name,
            package_flags,
            name_count,
            name_offset,
            export_count,
            export_offset,
            import_count,
            import_offset,
            depends_offset,
            import_export_guid_offset,
            import_guid_count,
            export_guid_count,
            thumbnail_table_offset,
            guid,
            generations,
            engine_version,
            cooker_version,
            compression_flags,
            summary_compressed_chunks,
            package_source,
            additional_packages,
            metadata_offset,
            garbage_size,
            compressed_chunks_offset,
            last_block_size,
        })
    }

    pub fn is_probable_rocket_league(&self) -> bool {
        self.magic == PACKAGE_TAG
            && self.name_offset >= 0
            && self.total_header_size > 0
            && self.licensee_version >= 20
    }

    pub fn encrypted_region_size(&self) -> Result<usize> {
        let total_header_size = i64::from(self.total_header_size);
        let garbage_size = i64::from(self.garbage_size);
        let name_offset = i64::from(self.name_offset);
        let base = total_header_size
            .checked_sub(garbage_size)
            .and_then(|value| value.checked_sub(name_offset))
            .ok_or_else(|| anyhow!("invalid encrypted table region size"))?;
        if base <= 0 {
            return Err(anyhow!("encrypted table region size must be positive"));
        }
        let aligned =
            (usize::try_from(base).map_err(|_| anyhow!("encrypted region overflow"))? + 15) & !15;
        Ok(aligned)
    }

    pub fn name_offset_usize(&self) -> Result<usize> {
        i32_to_usize(self.name_offset, "name_offset")
    }

    pub fn relative_to_name_offset(&self, absolute_offset: i32, label: &str) -> Result<usize> {
        let absolute_offset = i32_to_usize(absolute_offset, label)?;
        let name_offset = self.name_offset_usize()?;
        absolute_offset
            .checked_sub(name_offset)
            .ok_or_else(|| anyhow!("{label} is before the name table"))
    }

    pub fn compressed_chunks_offset_usize(&self) -> Result<usize> {
        i32_to_usize(self.compressed_chunks_offset, "compressed_chunks_offset")
    }

    pub fn table_key_sha256(&self) -> String {
        hash_bytes(&DEFAULT_TABLE_KEY)
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}

fn i32_to_usize(value: i32, label: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| anyhow!("{label} must be non-negative"))
}
