#[derive(Debug, Clone, Default)]
pub struct DependsEntry {
    pub index: i32,
}

#[derive(Debug, Clone, Default)]
pub struct DependsTable {
    pub entries: Vec<DependsEntry>,
}
