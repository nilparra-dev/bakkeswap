use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::database::DatabaseService;
use crate::domain::models::{PlanBlocker, PlanBuildReport, PlanWarning, SwapOperation, SwapPlan};
use crate::upk::{rebuild_target_identity, SandboxRebuildReport, SandboxRebuildValidationResult};

const COOKED_DIR_KEY: &str = "cooked_dir";
const SUPPORTED_PLAN_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildPlanRequest {
    pub plan_path: PathBuf,
    pub output_root: Option<PathBuf>,
    pub create_dir: bool,
}

#[derive(Debug, Clone)]
pub struct BuildService {
    database: DatabaseService,
}

#[derive(Debug, Default)]
struct OperationBuildOutcome {
    rebuild: Option<SandboxRebuildReport>,
    warnings: Vec<PlanWarning>,
    blockers: Vec<PlanBlocker>,
}

impl BuildService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn build_plan(&self, request: &BuildPlanRequest) -> Result<PlanBuildReport> {
        let plan_path = request.plan_path.clone();
        let mut plan = read_plan_file(&plan_path)?;
        let created_at = Utc::now();
        let build_id = build_id(&plan.plan_id, &created_at.to_rfc3339());
        let build_root = resolve_build_root(&self.database, &plan, request.output_root.as_deref());
        let plan_exists_in_db = self.plan_exists_in_database(&plan.plan_id).unwrap_or(false);

        let mut warnings = Vec::new();
        let mut blockers = Vec::new();
        let mut current_cooked_root = None;

        match self.database.get_string_setting(COOKED_DIR_KEY) {
            Ok(value) => {
                current_cooked_root = value;
            }
            Err(error) => warnings.push(warning(
                "configured_cooked_root_unavailable",
                format!("Could not read the current configured cooked root from SQLite: {error}"),
            )),
        }

        if plan.schema_version != SUPPORTED_PLAN_SCHEMA_VERSION {
            blockers.push(blocker(
                "unsupported_plan_schema",
                format!(
                    "Unsupported plan schema_version {}. This builder only supports schema_version {}.",
                    plan.schema_version, SUPPORTED_PLAN_SCHEMA_VERSION
                ),
            ));
        }
        if !plan.build_blockers.is_empty() {
            blockers.extend(plan.build_blockers.iter().cloned());
        }
        if let Some(mismatch_warning) = cooked_root_mismatch_warning(
            plan.configured_cooked_root.as_deref(),
            current_cooked_root.as_deref(),
        ) {
            warnings.push(mismatch_warning);
        }
        if !plan_exists_in_db {
            warnings.push(warning(
                "build_metadata_plan_not_persisted",
                "Plan is not present in the SQLite swap_plans table; build metadata will be written to the plan JSON only.",
            ));
        }

        let preflight_blocker_count = blockers.len();
        let mut visual_output_path = None;
        let mut visual_validation = None;
        let mut thumbnail_output_path = None;
        let mut thumbnail_validation = None;

        if blockers.is_empty() {
            let visual_index = operation_index(&plan, "visual");
            match visual_index {
                Some(index) => {
                    let outcome = build_operation(
                        &mut plan.operations[index],
                        &build_root,
                        request.create_dir,
                        current_cooked_root.clone(),
                        true,
                    );
                    warnings.extend(outcome.warnings);
                    blockers.extend(outcome.blockers);
                    if let Some(report) = outcome.rebuild {
                        visual_output_path = Some(report.output_path.clone());
                        visual_validation = Some(report.validation);
                    }
                }
                None => blockers.push(blocker(
                    "missing_visual_operation",
                    "The saved plan does not include a visual operation.".to_string(),
                )),
            }

            let thumbnail_index = operation_index(&plan, "thumbnail");
            if let Some(index) = thumbnail_index {
                let outcome = build_operation(
                    &mut plan.operations[index],
                    &build_root,
                    request.create_dir,
                    current_cooked_root.clone(),
                    false,
                );
                warnings.extend(outcome.warnings);
                blockers.extend(outcome.blockers);
                if let Some(report) = outcome.rebuild {
                    thumbnail_output_path = Some(report.output_path.clone());
                    thumbnail_validation = Some(report.validation);
                }
            } else {
                warnings.push(warning(
                    "thumbnail_operation_missing",
                    "The saved plan does not include a thumbnail operation. Visual-only build will continue.",
                ));
            }
        }

        let status = if blockers.is_empty() {
            "built"
        } else if preflight_blocker_count > 0 {
            "blocked"
        } else {
            "failed"
        }
        .to_string();

        let report = PlanBuildReport {
            build_id,
            profile_name: plan.profile_name.clone(),
            status: status.clone(),
            build_root: build_root.display().to_string(),
            visual_output_path,
            visual_validation,
            thumbnail_output_path,
            thumbnail_validation,
            warnings: warnings.clone(),
            blockers: blockers.clone(),
            no_install_performed: true,
            created_at,
        };

        plan.status = status;
        plan.last_build = Some(report.clone());
        write_plan_file(&plan_path, &plan)?;

        if plan_exists_in_db {
            if let Err(error) = self.persist_build_metadata(&plan, &report) {
                let mut amended_report = report.clone();
                amended_report.warnings.push(warning(
                    "build_metadata_persist_failed",
                    format!(
                        "Build completed but SQLite build metadata could not be persisted: {error}"
                    ),
                ));
                plan.last_build = Some(amended_report.clone());
                write_plan_file(&plan_path, &plan)?;
                return Ok(amended_report);
            }
        }

        Ok(report)
    }

    fn plan_exists_in_database(&self, plan_id: &str) -> Result<bool> {
        let connection = self.database.connect()?;
        let exists = connection
            .query_row(
                "SELECT plan_id FROM swap_plans WHERE plan_id = ?1 LIMIT 1",
                params![plan_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_some();
        Ok(exists)
    }

    fn persist_build_metadata(&self, plan: &SwapPlan, report: &PlanBuildReport) -> Result<()> {
        let connection = self.database.connect()?;
        let validation_json = serde_json::to_string(report)?;
        let validations = [
            report.visual_validation.as_ref(),
            report.thumbnail_validation.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<&SandboxRebuildValidationResult>>();
        let body_matches_source = !validations.is_empty()
            && validations
                .iter()
                .all(|validation| validation.body_equals_source);
        let target_identity_present = !validations.is_empty()
            && validations
                .iter()
                .all(|validation| validation.target_name_present);
        let modified_export_refs_detected = !validations.is_empty()
            && validations.iter().all(|validation| {
                validation.target_export_name_count >= validation.modified_export_indices.len()
            });

        connection.execute(
            "INSERT INTO builds (
                build_id, plan_id, build_root, visual_output_path, thumb_output_path,
                validation_json, body_matches_source, target_identity_present,
                modified_export_refs_detected, created_at, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                report.build_id,
                plan.plan_id,
                report.build_root,
                report.visual_output_path.as_deref(),
                report.thumbnail_output_path.as_deref(),
                validation_json,
                if body_matches_source { 1 } else { 0 },
                if target_identity_present { 1 } else { 0 },
                if modified_export_refs_detected { 1 } else { 0 },
                report.created_at.to_rfc3339(),
                report.status,
            ],
        )?;

        connection.execute(
            "UPDATE swap_plans
             SET status = ?2, notes_json = ?3
             WHERE plan_id = ?1",
            params![plan.plan_id, plan.status, plan_notes_json(plan)?],
        )?;

        Ok(())
    }
}

fn build_operation(
    operation: &mut SwapOperation,
    build_root: &Path,
    create_dir: bool,
    configured_cooked_dir: Option<String>,
    required: bool,
) -> OperationBuildOutcome {
    let mut outcome = OperationBuildOutcome::default();
    let kind_label = operation.kind.to_ascii_lowercase();

    if !operation.enabled {
        if required {
            outcome.blockers.push(blocker(
                "visual_operation_disabled",
                "The saved plan does not have a fully resolved visual operation to build."
                    .to_string(),
            ));
        } else {
            outcome.warnings.push(warning(
                "thumbnail_skipped",
                "Thumbnail build skipped because the saved plan does not have a fully resolved thumbnail operation.",
            ));
        }
        return outcome;
    }

    let source_path = match operation.source_path.as_deref() {
        Some(path) => PathBuf::from(path),
        None => {
            outcome.blockers.push(blocker(
                &format!("{}_source_path_missing", kind_label),
                format!(
                    "The {} operation is enabled but its source_path is missing from the plan.",
                    kind_label
                ),
            ));
            return outcome;
        }
    };
    let target_path = match operation.target_path.as_deref() {
        Some(path) => PathBuf::from(path),
        None => {
            outcome.blockers.push(blocker(
                &format!("{}_target_path_missing", kind_label),
                format!(
                    "The {} operation is enabled but its target_path is missing from the plan.",
                    kind_label
                ),
            ));
            return outcome;
        }
    };
    let output_path = match resolve_operation_output_path(build_root, operation) {
        Ok(path) => path,
        Err(error) => {
            outcome.blockers.push(blocker(
                &format!("{}_output_path_unresolved", kind_label),
                error.to_string(),
            ));
            return outcome;
        }
    };
    operation.output_path = Some(output_path.display().to_string());

    if !source_path.exists() {
        outcome.blockers.push(blocker(
            &format!("{}_source_missing", kind_label),
            format!(
                "The {} source package does not exist on disk: {}",
                kind_label,
                source_path.display()
            ),
        ));
    }
    if !target_path.exists() {
        outcome.blockers.push(blocker(
            &format!("{}_target_missing", kind_label),
            format!(
                "The {} target package does not exist on disk: {}",
                kind_label,
                target_path.display()
            ),
        ));
    }
    if !outcome.blockers.is_empty() {
        return outcome;
    }

    match rebuild_target_identity(
        &source_path,
        &target_path,
        &output_path,
        &crate::upk::SandboxRebuildOptions {
            create_dir,
            configured_cooked_dir,
            ..crate::upk::SandboxRebuildOptions::default()
        },
    ) {
        Ok(report) => {
            if !report.validation.passed {
                outcome.blockers.push(blocker(
                    &format!("{}_validation_failed", kind_label),
                    format!(
                        "The {} rebuild completed but its validation checks did not pass for {}.",
                        kind_label,
                        output_path.display()
                    ),
                ));
            }
            for validation_warning in &report.validation.warnings {
                outcome.warnings.push(warning(
                    &format!("{}_validation_warning", kind_label),
                    format!("{} build warning: {}", kind_label, validation_warning),
                ));
            }
            outcome.rebuild = Some(report);
        }
        Err(error) => outcome.blockers.push(blocker(
            &format!("{}_build_failed", kind_label),
            format!("{} build failed: {error}", kind_label),
        )),
    }

    outcome
}

fn read_plan_file(plan_path: &Path) -> Result<SwapPlan> {
    let payload = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read saved plan from {}", plan_path.display()))?;
    let mut plan: SwapPlan = serde_json::from_str(&payload)
        .with_context(|| format!("failed to parse saved plan from {}", plan_path.display()))?;
    plan.plan_path = plan_path.display().to_string();
    Ok(plan)
}

fn write_plan_file(plan_path: &Path, plan: &SwapPlan) -> Result<()> {
    if let Some(parent) = plan_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create plan directory while updating build results at {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(plan)?;
    fs::write(plan_path, format!("{payload}\n")).with_context(|| {
        format!(
            "failed to write updated plan build results to {}",
            plan_path.display()
        )
    })?;
    Ok(())
}

fn plan_notes_json(plan: &SwapPlan) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "status": plan.status,
        "warnings": plan.warnings,
        "build_blockers": plan.build_blockers,
        "compatibility": plan.compatibility,
        "operations": plan.operations,
        "database_path": plan.database_path,
        "configured_cooked_root": plan.configured_cooked_root,
        "rollback_notes": plan.rollback_notes,
        "last_build": plan.last_build,
    }))?)
}

fn resolve_build_root(
    database: &DatabaseService,
    plan: &SwapPlan,
    output_root: Option<&Path>,
) -> PathBuf {
    output_root.map(Path::to_path_buf).unwrap_or_else(|| {
        database
            .app_home()
            .join("workspace")
            .join("builds")
            .join(&plan.profile_name)
    })
}

fn resolve_operation_output_path(build_root: &Path, operation: &SwapOperation) -> Result<PathBuf> {
    let target_filename = operation.target_filename.as_deref().ok_or_else(|| {
        anyhow!(
            "The {} operation does not define a target_filename, so the sandbox output path cannot be resolved.",
            operation.kind
        )
    })?;
    Ok(build_root.join(target_filename))
}

fn operation_index(plan: &SwapPlan, kind: &str) -> Option<usize> {
    plan.operations
        .iter()
        .position(|operation| operation.kind == kind)
}

fn cooked_root_mismatch_warning(
    plan_cooked_root: Option<&str>,
    current_cooked_root: Option<&str>,
) -> Option<PlanWarning> {
    let matches = match (plan_cooked_root, current_cooked_root) {
        (Some(plan_root), Some(current_root)) => {
            normalize_compare_path(Path::new(plan_root))
                == normalize_compare_path(Path::new(current_root))
        }
        (None, None) => return None,
        _ => false,
    };

    if matches {
        return None;
    }

    Some(warning(
        "configured_cooked_root_mismatch",
        format!(
            "The current configured cooked root ({}) differs from the cooked root recorded in the plan ({}). The build will continue using the plan's saved source and target package paths.",
            current_cooked_root.unwrap_or("[not configured]"),
            plan_cooked_root.unwrap_or("[not recorded]")
        ),
    ))
}

fn normalize_compare_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    }

    if let Some(parent) = path.parent() {
        if parent.exists() {
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            if let Some(name) = path.file_name() {
                return canonical_parent.join(name);
            }
        }
    }

    path.to_path_buf()
}

fn build_id(plan_id: &str, created_at: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{plan_id}:{created_at}"));
    let digest = format!("{:x}", hasher.finalize());
    format!("build_{}", &digest[..16])
}

fn warning(code: &str, message: impl Into<String>) -> PlanWarning {
    PlanWarning {
        code: code.to_string(),
        message: message.into(),
    }
}

fn blocker(code: &str, message: String) -> PlanBlocker {
    PlanBlocker {
        code: code.to_string(),
        message,
    }
}
