use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub product_id: i64,
    pub name: String,
    pub slot: Option<String>,
    pub visual_upk: Option<String>,
    pub thumb_upk: Option<String>,
}

#[derive(Debug, Default)]
pub struct SearchEngine;

impl SearchEngine {
    pub fn search_products(&self, _request: &SearchRequest) -> Result<Vec<SearchHit>> {
        bail!("not implemented: product search")
    }
}
