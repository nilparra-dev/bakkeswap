use std::path::Path;

use anyhow::{bail, Result};

use super::{DependsTable, ExportTable, ImportTable, NameTable};

#[derive(Debug, Clone, Default)]
pub struct ParsedUpk {
    pub source_path: String,
    pub name_table: NameTable,
    pub import_table: ImportTable,
    pub export_table: ExportTable,
    pub depends_table: DependsTable,
}

#[derive(Debug, Default)]
pub struct UpkParser;

impl UpkParser {
    pub fn parse_file(&self, _path: &Path) -> Result<ParsedUpk> {
        bail!("not implemented: upk parser")
    }
}
