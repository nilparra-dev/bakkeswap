#[derive(Debug, Clone, Default)]
pub struct NameEntry {
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct NameTable {
    pub entries: Vec<NameEntry>,
}
