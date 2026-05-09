use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::format::{NameReference, PackageSummary};
use super::names::NameTable;
use super::reader::ByteReader;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportEntry {
    pub index: usize,
    pub class_index: i32,
    pub super_index: i32,
    pub outer_index: i32,
    pub object_name: NameReference,
    pub archetype_index: i32,
    pub object_flags: u64,
    pub serial_size: i32,
    pub serial_offset: i64,
    pub export_flags: i32,
    pub net_objects: Vec<i32>,
    pub package_guid: String,
    pub package_flags: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportTable {
    pub entries: Vec<ExportEntry>,
}

impl ExportTable {
    pub fn parse(decrypted: &[u8], summary: &PackageSummary, names: &NameTable) -> Result<Self> {
        let offset = summary.relative_to_name_offset(summary.export_offset, "export_offset")?;
        let mut reader = ByteReader::with_offset(decrypted, offset)?;
        let mut entries = Vec::with_capacity(summary.export_count);
        let use_i64_offset = summary.licensee_version >= 22;

        for index in 0..summary.export_count {
            let class_index = reader.read_i32()?;
            let super_index = reader.read_i32()?;
            let outer_index = reader.read_i32()?;
            let object_name = read_name_reference(&mut reader, names)?;
            let archetype_index = reader.read_i32()?;
            let object_flags = reader.read_u64()?;
            let serial_size = reader.read_i32()?;
            let serial_offset = if use_i64_offset {
                reader.read_i64()?
            } else {
                i64::from(reader.read_i32()?)
            };
            let export_flags = reader.read_i32()?;
            let net_count = reader.read_tarray_count()?;
            let mut net_objects = Vec::with_capacity(net_count);
            for _ in 0..net_count {
                net_objects.push(reader.read_i32()?);
            }
            let package_guid = reader
                .read_bytes(16)?
                .iter()
                .map(|value| format!("{value:02x}"))
                .collect();
            let package_flags = reader.read_i32()?;

            entries.push(ExportEntry {
                index,
                class_index,
                super_index,
                outer_index,
                object_name,
                archetype_index,
                object_flags,
                serial_size,
                serial_offset,
                export_flags,
                net_objects,
                package_guid,
                package_flags,
            });
        }

        Ok(Self { entries })
    }
}

fn read_name_reference(reader: &mut ByteReader<'_>, names: &NameTable) -> Result<NameReference> {
    let name_index = reader.read_i32()?;
    let instance_number = reader.read_i32()?;
    Ok(names.resolve_reference(name_index, instance_number))
}
