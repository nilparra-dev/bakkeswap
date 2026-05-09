#[derive(Debug, Clone, Default)]
pub struct ExportEntry {
    pub object_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExportTable {
    pub entries: Vec<ExportEntry>,
}
