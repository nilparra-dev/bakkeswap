use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::format::{NameReference, PackageSummary};
use super::reader::ByteReader;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameEntry {
    pub index: usize,
    pub offset: usize,
    pub name: String,
    pub flags: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameTable {
    pub entries: Vec<NameEntry>,
}

impl NameTable {
    pub fn parse(decrypted: &[u8], summary: &PackageSummary) -> Result<Self> {
        let mut reader = ByteReader::new(decrypted);
        let mut entries = Vec::with_capacity(summary.name_count);
        for index in 0..summary.name_count {
            let offset = reader.position();
            entries.push(NameEntry {
                index,
                offset,
                name: reader.read_fstring()?,
                flags: reader.read_u64()?,
            });
        }

        let expected_end =
            summary.relative_to_name_offset(summary.import_offset, "import_offset")?;
        if reader.position() != expected_end {
            return Err(anyhow!(
                "name table ended at decrypted offset 0x{:x}, expected 0x{:x}",
                reader.position(),
                expected_end
            ));
        }

        Ok(Self { entries })
    }

    pub fn resolve_reference(&self, name_index: i32, instance_number: i32) -> NameReference {
        let name = usize::try_from(name_index)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|entry| {
                if instance_number == 0 {
                    entry.name.clone()
                } else {
                    format!("{}_{}", entry.name, instance_number)
                }
            });

        NameReference {
            name_index,
            instance_number,
            name,
        }
    }
}
