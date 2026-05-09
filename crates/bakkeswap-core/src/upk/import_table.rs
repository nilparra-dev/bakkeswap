#[derive(Debug, Clone, Default)]
pub struct ImportEntry {
    pub object_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ImportTable {
    pub entries: Vec<ImportEntry>,
}
