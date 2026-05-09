use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::database::DatabaseService;
use crate::domain::models::{
    BuildMethod, CompatibilityCheck, LocalFileRecord, PlanBlocker, PlanWarning, PlannedProduct,
    ProductRecord, SwapOperation, SwapPlan, SwapPlanRecord,
};

const COOKED_DIR_KEY: &str = "cooked_dir";
const PLAN_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct PlannerService {
    database: DatabaseService,
}

#[derive(Debug, Clone)]
struct ResolvedPackage {
    filename: Option<String>,
    path: Option<String>,
    sha256: Option<String>,
    identity: Option<String>,
}

impl PlannerService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn create_plan(&self, target_product_id: i64, source_product_id: i64) -> Result<SwapPlan> {
        let connection = self.database.connect()?;
        let target = load_product(&connection, target_product_id)?.ok_or_else(|| {
            anyhow!(
                "target product {} was not found in the SQLite products table",
                target_product_id
            )
        })?;
        let source = load_product(&connection, source_product_id)?.ok_or_else(|| {
            anyhow!(
                "source product {} was not found in the SQLite products table",
                source_product_id
            )
        })?;

        let created_at = Utc::now();
        let configured_cooked_root = self.database.get_string_setting(COOKED_DIR_KEY)?;
        let workspace_root = self.database.app_home().join("workspace");
        let profile_name = make_profile_name(&source.name, &target.name);
        let plan_dir = workspace_root.join("plans").join(&profile_name);
        let plan_path = plan_dir.join("swap_plan.json");
        let backup_dir = workspace_root.join("backups").join(&profile_name);
        let build_dir = workspace_root.join("builds").join(&profile_name);

        let target_visual = resolve_package(
            &connection,
            &target,
            false,
            configured_cooked_root.as_deref(),
        )?;
        let target_thumb = resolve_package(
            &connection,
            &target,
            true,
            configured_cooked_root.as_deref(),
        )?;
        let source_visual = resolve_package(
            &connection,
            &source,
            false,
            configured_cooked_root.as_deref(),
        )?;
        let source_thumb = resolve_package(
            &connection,
            &source,
            true,
            configured_cooked_root.as_deref(),
        )?;

        let compatibility = CompatibilityCheck {
            same_slot: same_slot(&target, &source),
        };

        let mut warnings = vec![
            warning(
                "offline_only",
                "Offline/local only. Do not use this tool online and do not bypass EAC.",
            ),
            warning(
                "plan_only",
                "Planning only. This command writes plan metadata and does not rebuild, install, or modify CookedPCConsole.",
            ),
        ];
        let mut build_blockers = Vec::new();

        if !compatibility.same_slot {
            build_blockers.push(blocker(
                "slot_mismatch",
                format!(
                    "Slot mismatch: target={} source={}",
                    target.slot.as_deref().unwrap_or("Unknown"),
                    source.slot.as_deref().unwrap_or("Unknown")
                ),
            ));
        }

        if !is_swappable_product(&target) {
            build_blockers.push(blocker(
                "target_not_swappable",
                format!(
                    "Target product {} is not a swappable package-backed product",
                    target.name
                ),
            ));
        }
        if !is_swappable_product(&source) {
            build_blockers.push(blocker(
                "source_not_swappable",
                format!(
                    "Source product {} is not a swappable package-backed product",
                    source.name
                ),
            ));
        }

        if target_visual.path.is_none() {
            build_blockers.push(blocker(
                "missing_target_visual_package",
                format!(
                    "Target visual package is not indexed locally: {}",
                    target_visual.filename.as_deref().unwrap_or("[unresolved]")
                ),
            ));
        }
        if source_visual.path.is_none() {
            build_blockers.push(blocker(
                "missing_source_visual_package",
                format!(
                    "Source visual package is not indexed locally: {}",
                    source_visual.filename.as_deref().unwrap_or("[unresolved]")
                ),
            ));
        }

        let visual_operation = build_operation(
            "visual",
            &source_visual,
            &target_visual,
            &backup_dir,
            &build_dir,
        );
        let thumbnail_operation = build_operation(
            "thumbnail",
            &source_thumb,
            &target_thumb,
            &backup_dir,
            &build_dir,
        );

        if thumbnail_operation.enabled {
            warnings.push(warning(
                "thumbnail_ready",
                "Thumbnail swap planning is enabled because both thumbnail packages exist locally.",
            ));
        } else {
            warnings.push(warning(
                "thumbnail_unavailable",
                "Thumbnail swap planning is disabled because one or both thumbnail packages are missing locally.",
            ));
        }

        let target_product = planned_product(&target, &target_visual, &target_thumb);
        let source_product = planned_product(&source, &source_visual, &source_thumb);

        let rollback_notes = [visual_operation.clone(), thumbnail_operation.clone()]
            .into_iter()
            .filter(|operation| operation.enabled)
            .filter_map(|operation| {
                operation
                    .backup_path
                    .as_ref()
                    .zip(operation.target_filename.as_ref())
                    .map(|(backup_path, target_filename)| {
                        format!("Restore {target_filename} from {backup_path}")
                    })
            })
            .collect::<Vec<_>>();

        let plan_status = if build_blockers.is_empty() {
            "planned"
        } else {
            "blocked"
        }
        .to_string();

        let plan = SwapPlan {
            plan_id: build_plan_id(
                target_product_id,
                source_product_id,
                &created_at.to_rfc3339(),
            ),
            schema_version: PLAN_SCHEMA_VERSION,
            created_at,
            profile_name: profile_name.clone(),
            offline_only: true,
            database_path: Some(self.database.database_path().display().to_string()),
            configured_cooked_root: configured_cooked_root.clone(),
            status: plan_status.clone(),
            target_product,
            source_product,
            compatibility,
            operations: vec![visual_operation.clone(), thumbnail_operation.clone()],
            warnings,
            build_blockers,
            last_build: None,
            rollback_notes,
            plan_path: plan_path.display().to_string(),
        };

        write_plan_file(&plan_path, &plan)?;
        persist_plan(
            &connection,
            &plan,
            SwapPlanRecord {
                plan_id: plan.plan_id.clone(),
                profile_name,
                target_product_id,
                source_product_id,
                build_method: BuildMethod::TargetIdentityRebuild,
                target_visual_upk: visual_operation.target_filename.clone(),
                target_thumb_upk: thumbnail_operation.target_filename.clone(),
                source_visual_upk: visual_operation.source_filename.clone(),
                source_thumb_upk: thumbnail_operation.source_filename.clone(),
                target_visual_identity: visual_operation.target_identity.clone(),
                target_thumb_identity: thumbnail_operation.target_identity.clone(),
                created_at: plan.created_at,
                status: plan_status,
            },
        )?;

        Ok(plan)
    }
}

fn load_product(connection: &Connection, product_id: i64) -> Result<Option<ProductRecord>> {
    connection
        .query_row(
            "SELECT product_id, name, slot, slot_id, quality, paintable, visual_upk, thumb_upk,
                    visual_asset, thumbnail_asset, product_asset_package, product_asset_path,
                    product_thumbnail_package, product_thumbnail_asset
             FROM products
             WHERE product_id = ?1",
            params![product_id],
            |row| {
                Ok(ProductRecord {
                    product_id: row.get(0)?,
                    name: row.get(1)?,
                    slot: row.get(2)?,
                    slot_id: row.get(3)?,
                    quality: row.get(4)?,
                    paintable: row.get::<_, i64>(5)? != 0,
                    visual_upk: row.get(6)?,
                    thumb_upk: row.get(7)?,
                    visual_asset: row.get(8)?,
                    thumbnail_asset: row.get(9)?,
                    product_asset_package: row.get(10)?,
                    product_asset_path: row.get(11)?,
                    product_thumbnail_package: row.get(12)?,
                    product_thumbnail_asset: row.get(13)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn resolve_package(
    connection: &Connection,
    product: &ProductRecord,
    thumbnail: bool,
    cooked_root: Option<&str>,
) -> Result<ResolvedPackage> {
    let candidate_filename = if thumbnail {
        expected_thumb_filename(product)
    } else {
        expected_visual_filename(product)
    };
    let local_file = match candidate_filename.as_deref() {
        Some(filename) => load_local_file(connection, filename, cooked_root)?,
        None => None,
    };
    let filename = local_file
        .as_ref()
        .map(|entry| entry.filename.clone())
        .or(candidate_filename);

    Ok(ResolvedPackage {
        identity: filename.as_deref().and_then(identity_from_filename),
        path: local_file.as_ref().map(|entry| entry.path.clone()),
        sha256: local_file.as_ref().and_then(|entry| entry.sha256.clone()),
        filename,
    })
}

fn load_local_file(
    connection: &Connection,
    filename: &str,
    cooked_root: Option<&str>,
) -> Result<Option<LocalFileRecord>> {
    let row_mapper = |row: &rusqlite::Row<'_>| {
        Ok(LocalFileRecord {
            path: row.get(0)?,
            filename: row.get(1)?,
            kind: row.get(2)?,
            exists_on_disk: row.get::<_, i64>(3)? != 0,
            size_bytes: row
                .get::<_, Option<i64>>(4)?
                .map(|value| value.max(0) as u64),
            sha256: row.get(5)?,
            cooked_root: row.get(6)?,
        })
    };

    if let Some(cooked_root) = cooked_root {
        return connection
            .query_row(
                "SELECT path, filename, kind, exists_on_disk, size_bytes, sha256, cooked_root
                 FROM local_files
                 WHERE lower(filename) = lower(?1) AND cooked_root = ?2 AND exists_on_disk = 1
                 LIMIT 1",
                params![filename, cooked_root],
                row_mapper,
            )
            .optional()
            .map_err(Into::into);
    }

    connection
        .query_row(
            "SELECT path, filename, kind, exists_on_disk, size_bytes, sha256, cooked_root
             FROM local_files
             WHERE lower(filename) = lower(?1) AND exists_on_disk = 1
             ORDER BY updated_at DESC
             LIMIT 1",
            params![filename],
            row_mapper,
        )
        .optional()
        .map_err(Into::into)
}

fn expected_visual_filename(product: &ProductRecord) -> Option<String> {
    product
        .visual_upk
        .clone()
        .or_else(|| {
            product
                .product_asset_package
                .as_deref()
                .map(ensure_upk_extension)
        })
        .or_else(|| {
            product
                .product_asset_path
                .as_deref()
                .and_then(package_name_from_path)
                .map(|value| ensure_upk_extension(&value))
        })
}

fn expected_thumb_filename(product: &ProductRecord) -> Option<String> {
    product
        .thumb_upk
        .clone()
        .or_else(|| {
            product
                .product_thumbnail_package
                .as_deref()
                .map(ensure_upk_extension)
        })
        .or_else(|| {
            product
                .product_thumbnail_asset
                .as_deref()
                .and_then(package_name_from_path)
                .map(|value| ensure_upk_extension(&value))
        })
}

fn planned_product(
    product: &ProductRecord,
    visual: &ResolvedPackage,
    thumbnail: &ResolvedPackage,
) -> PlannedProduct {
    PlannedProduct {
        id: product.product_id,
        name: product.name.clone(),
        slot: product.slot.clone(),
        slot_id: product.slot_id,
        quality: product.quality.clone(),
        paintable: product.paintable,
        visual_upk: visual.filename.clone(),
        thumb_upk: thumbnail.filename.clone(),
        visual_asset: product.visual_asset.clone(),
        thumbnail_asset: product.thumbnail_asset.clone(),
    }
}

fn build_operation(
    kind: &str,
    source: &ResolvedPackage,
    target: &ResolvedPackage,
    backup_dir: &Path,
    build_dir: &Path,
) -> SwapOperation {
    let backup_path = target
        .filename
        .as_ref()
        .map(|filename| backup_dir.join(filename).display().to_string());
    let output_path = target
        .filename
        .as_ref()
        .map(|filename| build_dir.join(filename).display().to_string());

    SwapOperation {
        kind: kind.to_string(),
        enabled: source.filename.is_some()
            && target.filename.is_some()
            && source.path.is_some()
            && target.path.is_some()
            && source.identity.is_some()
            && target.identity.is_some(),
        source_filename: source.filename.clone(),
        target_filename: target.filename.clone(),
        source_identity: source.identity.clone(),
        target_identity: target.identity.clone(),
        source_path: source.path.clone(),
        target_path: target.path.clone(),
        source_sha256: source.sha256.clone(),
        target_sha256: target.sha256.clone(),
        backup_path,
        output_path,
    }
}

fn persist_plan(connection: &Connection, plan: &SwapPlan, record: SwapPlanRecord) -> Result<()> {
    let notes_json = serde_json::to_string(&json!({
        "warnings": plan.warnings,
        "build_blockers": plan.build_blockers,
        "compatibility": plan.compatibility,
        "operations": plan.operations,
        "database_path": plan.database_path,
        "configured_cooked_root": plan.configured_cooked_root,
        "rollback_notes": plan.rollback_notes,
    }))?;

    connection.execute(
        "INSERT INTO swap_plans (
            plan_id, profile_name, target_product_id, source_product_id,
            target_visual_upk, target_thumb_upk, source_visual_upk, source_thumb_upk,
            target_visual_identity, target_thumb_identity, build_method, plan_path,
            cooked_root, notes_json, created_at, status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            record.plan_id,
            record.profile_name,
            record.target_product_id,
            record.source_product_id,
            record.target_visual_upk,
            record.target_thumb_upk,
            record.source_visual_upk,
            record.source_thumb_upk,
            record.target_visual_identity,
            record.target_thumb_identity,
            record.build_method.as_str(),
            plan.plan_path.as_str(),
            plan.configured_cooked_root.as_deref(),
            notes_json,
            record.created_at.to_rfc3339(),
            record.status,
        ],
    )?;

    Ok(())
}

fn write_plan_file(plan_path: &Path, plan: &SwapPlan) -> Result<()> {
    if let Some(parent) = plan_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create plan output directory at {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(plan)?;
    fs::write(plan_path, format!("{payload}\n"))
        .with_context(|| format!("failed to write planner output to {}", plan_path.display()))?;
    Ok(())
}

fn same_slot(target: &ProductRecord, source: &ProductRecord) -> bool {
    match (target.slot_id, source.slot_id) {
        (Some(left), Some(right)) => left == right,
        _ => target
            .slot
            .as_deref()
            .zip(source.slot.as_deref())
            .map(|(left, right)| left.eq_ignore_ascii_case(right))
            .unwrap_or(false),
    }
}

fn is_swappable_product(product: &ProductRecord) -> bool {
    let is_title_slot = product
        .slot
        .as_deref()
        .map(|slot| slot.eq_ignore_ascii_case("Player Title"))
        .unwrap_or(false);

    !is_title_slot && expected_visual_filename(product).is_some()
}

fn identity_from_filename(filename: &str) -> Option<String> {
    let stem = Path::new(filename)
        .file_stem()?
        .to_string_lossy()
        .to_string();
    if stem.to_ascii_lowercase().ends_with("_sf") {
        return Some(stem[..stem.len().saturating_sub(3)].to_string());
    }
    Some(stem)
}

fn package_name_from_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
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

fn ensure_upk_extension(value: &str) -> String {
    if value.to_ascii_lowercase().ends_with(".upk") {
        value.to_string()
    } else {
        format!("{value}.upk")
    }
}

fn make_profile_name(source_name: &str, target_name: &str) -> String {
    format!(
        "{}_on_{}",
        compact_slug(source_name),
        compact_slug(target_name)
    )
}

fn compact_slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
        } else if !output.is_empty() && !output.ends_with('_') {
            output.push('_');
        }
    }

    let trimmed = output.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
}

fn build_plan_id(target_product_id: i64, source_product_id: i64, created_at: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "{target_product_id}:{source_product_id}:{created_at}"
    ));
    let digest = format!("{:x}", hasher.finalize());
    format!("plan_{}", &digest[..16])
}

fn warning(code: &str, message: &str) -> PlanWarning {
    PlanWarning {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn blocker(code: &str, message: String) -> PlanBlocker {
    PlanBlocker {
        code: code.to_string(),
        message,
    }
}
