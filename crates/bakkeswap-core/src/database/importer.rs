use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::DatabaseService;

const CODERED_DUMPS_DIR_KEY: &str = "codered_dumps_dir";
const PRODUCT_DUMP_FILE: &str = "ProductDump.json";
const SLOT_DUMP_FILE: &str = "SlotDump.json";
const PAINT_DUMP_FILE: &str = "PaintDump.json";
const TITLE_DUMP_FILE: &str = "TitleDump.json";

type JsonRecord = Map<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRedImportSource {
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseImportSummary {
    pub source_dir: String,
    pub imported_products: usize,
    pub imported_slots: usize,
    pub imported_paints: usize,
    pub imported_titles: usize,
}

#[derive(Debug, Clone)]
pub struct DatabaseImporter {
    database: DatabaseService,
}

impl DatabaseImporter {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn import_codered(&self, source: &CodeRedImportSource) -> Result<DatabaseImportSummary> {
        let source_dir = PathBuf::from(&source.folder);
        if !source_dir.exists() {
            return Err(anyhow!(
                "CodeRed dumps directory not found: {}",
                source_dir.display()
            ));
        }
        if !source_dir.is_dir() {
            return Err(anyhow!(
                "CodeRed dumps path is not a directory: {}",
                source_dir.display()
            ));
        }

        let product_records = read_dump_records(&source_dir.join(PRODUCT_DUMP_FILE), true)?;
        let slot_records = read_dump_records(&source_dir.join(SLOT_DUMP_FILE), false)?;
        let paint_records = read_dump_records(&source_dir.join(PAINT_DUMP_FILE), false)?;
        let title_records = read_dump_records(&source_dir.join(TITLE_DUMP_FILE), false)?;

        let slot_labels = build_slot_label_map(&slot_records);
        let connection = self.database.connect()?;
        let transaction = connection.unchecked_transaction()?;

        transaction.execute("DELETE FROM products", [])?;
        transaction.execute("DELETE FROM slots", [])?;
        transaction.execute("DELETE FROM paints", [])?;
        transaction.execute("DELETE FROM titles", [])?;

        let imported_slots = insert_slots(&transaction, &slot_records)?;
        let imported_paints = insert_paints(&transaction, &paint_records)?;
        let imported_titles = insert_titles(&transaction, &title_records)?;
        let imported_products = insert_products(&transaction, &product_records, &slot_labels)?;

        transaction.commit()?;
        self.database
            .set_string_setting(CODERED_DUMPS_DIR_KEY, &source_dir.display().to_string())?;

        Ok(DatabaseImportSummary {
            source_dir: source_dir.display().to_string(),
            imported_products,
            imported_slots,
            imported_paints,
            imported_titles,
        })
    }

    pub fn refresh(&self) -> Result<DatabaseImportSummary> {
        let configured_source = self
            .database
            .get_string_setting(CODERED_DUMPS_DIR_KEY)?
            .ok_or_else(|| anyhow!("no CodeRed dumps folder has been configured yet; run 'bakkeswap db import-codered <folder>' first"))?;

        self.import_codered(&CodeRedImportSource {
            folder: configured_source,
        })
    }
}

fn read_dump_records(path: &Path, required: bool) -> Result<Vec<JsonRecord>> {
    if !path.exists() {
        if required {
            return Err(anyhow!(
                "required CodeRed dump not found: {}",
                path.display()
            ));
        }
        return Ok(Vec::new());
    }

    let file_text = fs::read_to_string(path)
        .with_context(|| format!("failed to read CodeRed dump: {}", path.display()))?;
    let value: Value = serde_json::from_str(&file_text)
        .with_context(|| format!("failed to parse JSON dump: {}", path.display()))?;
    let records = value
        .as_array()
        .ok_or_else(|| anyhow!("expected a JSON array in dump: {}", path.display()))?
        .iter()
        .filter_map(|value| value.as_object().cloned())
        .collect();
    Ok(records)
}

fn insert_slots(transaction: &rusqlite::Transaction<'_>, records: &[JsonRecord]) -> Result<usize> {
    let mut inserted = 0usize;
    for record in records {
        let Some(slot_id) = find_i64(record, &["Slot Index", "slot_index"]) else {
            continue;
        };
        let name = find_string(record, &["Slot Label", "slot_label"])
            .unwrap_or_else(|| format!("Slot_{slot_id}"));
        let label = find_string(record, &["Slot Label", "slot_label"]);
        let plural_label = find_string(record, &["Slot Plural Label", "slot_plural_label"]);
        let description = find_string(record, &["Slot Description", "slot_description"]);

        transaction.execute(
            "INSERT INTO slots (slot_id, name, label, plural_label, description, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                slot_id,
                name,
                label,
                plural_label,
                description,
                Utc::now().to_rfc3339()
            ],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

fn insert_paints(transaction: &rusqlite::Transaction<'_>, records: &[JsonRecord]) -> Result<usize> {
    let mut inserted = 0usize;
    for record in records {
        let Some(paint_id) = find_i64(record, &["Paint Database Id", "database_paint_id"]) else {
            continue;
        };
        let Some(name) = find_string(record, &["Paint Database Name", "database_paint_name"])
        else {
            continue;
        };
        let label = find_string(record, &["Paint Database Label", "database_paint_label"])
            .unwrap_or_else(|| name.clone());
        let colors = record
            .get("Paint Database Colors")
            .or_else(|| record.get("database_paint_colors"))
            .map(serde_json::to_string)
            .transpose()?;

        transaction.execute(
            "INSERT INTO paints (paint_id, name, label, colors, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![paint_id, name, label, colors, Utc::now().to_rfc3339()],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

fn insert_titles(transaction: &rusqlite::Transaction<'_>, records: &[JsonRecord]) -> Result<usize> {
    let mut inserted = 0usize;
    for record in records {
        let Some(title_id) = find_string(record, &["Title Database Id", "database_title_id"])
        else {
            continue;
        };
        let title_text = find_string(record, &["Title Database Text", "database_title_text"])
            .unwrap_or_else(|| title_id.clone());
        let category = find_string(
            record,
            &["Title Database Category", "database_title_category"],
        );
        let color = find_string(record, &["Title Database Color", "database_title_color"]);
        let glow_color = find_string(
            record,
            &["Title Database GlowColor", "database_title_glowcolor"],
        );
        let sort_priority = find_i64(
            record,
            &[
                "Title Database Sort Priority",
                "database_title_sort_priority",
            ],
        );

        transaction.execute(
            "INSERT INTO titles (title_id, title_text, category, color, glow_color, sort_priority, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                title_id,
                title_text,
                category,
                color,
                glow_color,
                sort_priority,
                Utc::now().to_rfc3339(),
            ],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

fn insert_products(
    transaction: &rusqlite::Transaction<'_>,
    records: &[JsonRecord],
    slot_labels: &HashMap<i64, String>,
) -> Result<usize> {
    let mut inserted = 0usize;
    for record in records {
        let Some(product_id) = find_i64(record, &["Product Id", "product_id"]) else {
            continue;
        };
        let Some(name) = find_string(
            record,
            &[
                "Product Long Label",
                "Product Short Sort Label",
                "Product Label",
                "product_long_label",
                "product_sort_label",
                "product_label",
            ],
        ) else {
            continue;
        };
        let slot_id = find_i64(record, &["Slot Index", "product_slot_id"]);
        let slot = slot_id.and_then(|id| slot_labels.get(&id).cloned());
        let quality = find_string(record, &["Product Quality Label", "product_quality_label"])
            .or_else(|| {
                find_i64(record, &["Product Quality Id", "product_quality_id"])
                    .map(|value| value.to_string())
            });
        let paintable =
            find_bool(record, &["Product Paintable", "product_bool_paintable"]).unwrap_or(false);

        let product_asset_package =
            find_string(record, &["Product Asset Package", "product_asset_package"]);
        let product_asset_path = find_string(record, &["Product Asset Path", "product_asset_path"]);
        let product_thumbnail_package = find_string(
            record,
            &["Product Thumbnail Package", "product_thumbnail_package"],
        );
        let product_thumbnail_asset = find_string(
            record,
            &["Product Thumbnail Asset", "product_thumbnail_asset"],
        );
        let visual_upk = derive_upk_filename(
            product_asset_package.as_deref(),
            product_asset_path.as_deref(),
        );
        let thumb_upk = derive_upk_filename(
            product_thumbnail_package.as_deref(),
            product_thumbnail_asset.as_deref(),
        );
        let visual_asset = extract_asset_name(product_asset_path.as_deref())
            .or_else(|| product_asset_package.clone());
        let thumbnail_asset = extract_asset_name(product_thumbnail_asset.as_deref())
            .or_else(|| product_thumbnail_package.clone());

        transaction.execute(
            "INSERT INTO products (
                product_id, name, slot, slot_id, quality, paintable, visual_upk, thumb_upk,
                visual_asset, thumbnail_asset, product_asset_package, product_asset_path,
                product_thumbnail_package, product_thumbnail_asset, source_dump, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                product_id,
                name,
                slot,
                slot_id,
                quality,
                if paintable { 1 } else { 0 },
                visual_upk,
                thumb_upk,
                visual_asset,
                thumbnail_asset,
                product_asset_package,
                product_asset_path,
                product_thumbnail_package,
                product_thumbnail_asset,
                PRODUCT_DUMP_FILE,
                Utc::now().to_rfc3339(),
            ],
        )?;
        inserted += 1;
    }
    Ok(inserted)
}

fn build_slot_label_map(records: &[JsonRecord]) -> HashMap<i64, String> {
    let mut labels = HashMap::new();
    for record in records {
        let Some(slot_id) = find_i64(record, &["Slot Index", "slot_index"]) else {
            continue;
        };
        if let Some(label) = find_string(record, &["Slot Label", "slot_label"]) {
            labels.insert(slot_id, label);
        }
    }
    labels
}

fn derive_upk_filename(package_name: Option<&str>, fallback_value: Option<&str>) -> Option<String> {
    let stem = package_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| extract_package_stem(fallback_value));

    stem.map(|value| {
        if value.to_ascii_lowercase().ends_with(".upk") {
            value
        } else {
            format!("{value}.upk")
        }
    })
}

fn extract_package_stem(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .split('.')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .to_string(),
    )
}

fn extract_asset_name(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .rsplit('.')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .to_string(),
    )
}

fn find_string(record: &JsonRecord, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(value) = record.get(*key) else {
            continue;
        };
        match value {
            Value::String(text) if !text.trim().is_empty() => return Some(text.trim().to_string()),
            Value::Number(number) => return Some(number.to_string()),
            Value::Bool(value) => return Some(value.to_string()),
            _ => {}
        }
    }
    None
}

fn find_i64(record: &JsonRecord, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(value) = record.get(*key) else {
            continue;
        };
        match value {
            Value::Number(number) => return number.as_i64(),
            Value::String(text) => {
                if let Ok(parsed) = text.trim().parse::<i64>() {
                    return Some(parsed);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_bool(record: &JsonRecord, keys: &[&str]) -> Option<bool> {
    for key in keys {
        let Some(value) = record.get(*key) else {
            continue;
        };
        match value {
            Value::Bool(boolean) => return Some(*boolean),
            Value::Number(number) => return Some(number.as_i64().unwrap_or_default() != 0),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => return Some(true),
                "false" | "0" | "no" => return Some(false),
                _ => {}
            },
            _ => {}
        }
    }
    None
}
