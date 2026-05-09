use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::format::{NameReference, PackageSummary};
use super::names::NameTable;
use super::reader::ByteReader;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportEntry {
    pub index: usize,
    pub class_package: NameReference,
    pub class_name: NameReference,
    pub outer_index: i32,
    pub object_name: NameReference,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportTable {
    pub entries: Vec<ImportEntry>,
}

impl ImportTable {
    pub fn parse(decrypted: &[u8], summary: &PackageSummary, names: &NameTable) -> Result<Self> {
        let offset = summary.relative_to_name_offset(summary.import_offset, "import_offset")?;
        let mut reader = ByteReader::with_offset(decrypted, offset)?;
        let mut entries = Vec::with_capacity(summary.import_count);
        for index in 0..summary.import_count {
            entries.push(ImportEntry {
                index,
                class_package: read_name_reference(&mut reader, names)?,
                class_name: read_name_reference(&mut reader, names)?,
                outer_index: reader.read_i32()?,
                object_name: read_name_reference(&mut reader, names)?,
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
