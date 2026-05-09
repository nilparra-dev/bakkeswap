use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::DatabaseService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    Product,
    Title,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub name: String,
    pub slot: Option<String>,
    pub product_asset_package: Option<String>,
    pub product_thumbnail_package: Option<String>,
    pub swappable: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchEngine {
    database: DatabaseService,
}

impl SearchEngine {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn search_products(&self, request: &SearchRequest) -> Result<Vec<SearchHit>> {
        let query = request.query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.database.connect()?;
        let wildcard = format!("%{}%", query.to_ascii_lowercase());
        let exact = query.to_string();
        let product_limit = request.limit.max(1) as i64;
        let title_limit = request.limit.max(1) as i64;

        let mut hits = Vec::new();

        let mut product_statement = connection.prepare(
            "SELECT product_id, name, slot, product_asset_package, product_thumbnail_package
             FROM products
             WHERE CAST(product_id AS TEXT) = ?1
                OR lower(name) LIKE ?2
                OR lower(COALESCE(slot, '')) LIKE ?2
                OR lower(COALESCE(product_asset_package, '')) LIKE ?2
                OR lower(COALESCE(product_thumbnail_package, '')) LIKE ?2
                OR lower(COALESCE(product_asset_path, '')) LIKE ?2
                OR lower(COALESCE(product_thumbnail_asset, '')) LIKE ?2
             ORDER BY CASE WHEN CAST(product_id AS TEXT) = ?1 THEN 0 ELSE 1 END, name COLLATE NOCASE
             LIMIT ?3",
        )?;
        let product_rows =
            product_statement.query_map(params![exact, wildcard, product_limit], |row| {
                Ok(SearchHit {
                    kind: SearchKind::Product,
                    id: row.get::<_, i64>(0)?.to_string(),
                    name: row.get(1)?,
                    slot: row.get(2)?,
                    product_asset_package: row.get(3)?,
                    product_thumbnail_package: row.get(4)?,
                    swappable: true,
                    note: None,
                })
            })?;
        for hit in product_rows {
            hits.push(hit?);
        }

        let mut title_statement = connection.prepare(
            "SELECT title_id, title_text
             FROM titles
             WHERE lower(title_id) LIKE ?1 OR lower(COALESCE(title_text, '')) LIKE ?1
             ORDER BY title_text COLLATE NOCASE, title_id COLLATE NOCASE
             LIMIT ?2",
        )?;
        let title_rows = title_statement.query_map(params![wildcard, title_limit], |row| {
            Ok(SearchHit {
                kind: SearchKind::Title,
                id: row.get(0)?,
                name: row.get(1)?,
                slot: None,
                product_asset_package: None,
                product_thumbnail_package: None,
                swappable: false,
                note: Some(
                    "Title metadata only; Player Title runtime support is out of scope for v1"
                        .to_string(),
                ),
            })
        })?;
        for hit in title_rows {
            hits.push(hit?);
        }

        if hits.len() > request.limit {
            hits.truncate(request.limit);
        }

        Ok(hits)
    }
}
