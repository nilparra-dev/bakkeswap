use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::backups::{PermanentOriginalBackupManager, ProfileBackupManager};
use crate::database::DatabaseService;
use crate::domain::models::{
    BackupPreview, BackupResult, InstallBlocker, InstallPreview, InstallPreviewFile, InstallReport,
    InstallWarning, InstalledFileRecord, OriginalBackupManifest, PlanBuildReport, SwapOperation,
    SwapPlan,
};

const COOKED_DIR_KEY: &str = "cooked_dir";
const INSTALL_MANIFEST_FILENAME: &str = "install_manifest.json";
const SUPPORTED_PLAN_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallPreviewRequest {
    pub plan_path: PathBuf,
    pub build_report: Option<PlanBuildReport>,
    pub configured_cooked_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallExecutionRequest {
    pub plan_path: PathBuf,
    pub build_report: Option<PlanBuildReport>,
    pub configured_cooked_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub confirmation: Option<String>,
    pub overwrite_profile_backup: bool,
}

#[derive(Debug, Clone)]
pub struct InstallerService {
    database: DatabaseService,
}

struct InstallPreviewContext {
    plan: SwapPlan,
    preview: InstallPreview,
    plan_exists_in_db: bool,
}

impl InstallerService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn preview_install(&self, request: &InstallPreviewRequest) -> Result<InstallPreview> {
        if !request.dry_run {
            bail!("Real install is not implemented yet. Use --dry-run.");
        }

        Ok(self.build_preview_context(request)?.preview)
    }

    pub fn install(&self, request: &InstallExecutionRequest) -> Result<InstallReport> {
        let preview_request = InstallPreviewRequest {
            plan_path: request.plan_path.clone(),
            build_report: request.build_report.clone(),
            configured_cooked_root: request.configured_cooked_root.clone(),
            workspace_root: request.workspace_root.clone(),
            dry_run: true,
        };
        let context = self.build_preview_context(&preview_request)?;
        let mut report = initialize_install_report(
            &context.plan,
            &context.preview,
            request.overwrite_profile_backup,
        );

        if context.preview.status != "preview_ready" || !context.preview.blockers.is_empty() {
            report.blockers.extend(context.preview.blockers.clone());
            report.status = "blocked".to_string();
            return Ok(report);
        }

        match request.confirmation.as_deref() {
            Some(value) if value.trim() == context.preview.confirmation_phrase => {}
            Some(_) => {
                report.blockers.push(blocker(
                    "confirmation_mismatch",
                    format!(
                        "Confirmation phrase did not match. Type '{}' exactly to continue.",
                        context.preview.confirmation_phrase
                    ),
                ));
                report.status = "blocked".to_string();
                return Ok(report);
            }
            None => {
                report.blockers.push(blocker(
                    "confirmation_required",
                    format!(
                        "Install requires explicit confirmation. Pass --confirm '{}' or type it interactively when prompted.",
                        context.preview.confirmation_phrase
                    ),
                ));
                report.status = "blocked".to_string();
                return Ok(report);
            }
        }

        let original_backup_result = PermanentOriginalBackupManager::new(self.database.clone())
            .prepare_from_preview(&context.preview)?;
        merge_backup_result(
            &mut report,
            &original_backup_result,
            "original_backup_prepare",
        );
        if !report.blockers.is_empty() {
            report.status = "blocked".to_string();
            return Ok(report);
        }

        let profile_backup_result = ProfileBackupManager::new(self.database.clone())
            .prepare_from_preview(&context.preview, request.overwrite_profile_backup)?;
        merge_backup_result(
            &mut report,
            &profile_backup_result,
            "profile_backup_prepare",
        );
        if !report.blockers.is_empty() {
            report.status = "blocked".to_string();
            return Ok(report);
        }

        let cooked_root = PathBuf::from(&context.preview.configured_cooked_root);
        let mut installed_any_file = false;
        for file in &mut report.files {
            let target_path = PathBuf::from(&file.target_path);
            let built_path = PathBuf::from(&file.built_path);

            if let Some(path_blocker) =
                validate_install_copy_paths(&file.kind, &cooked_root, &target_path, &built_path)
            {
                report.blockers.push(path_blocker);
                break;
            }

            if let Err(error) = copy_file(&built_path, &target_path) {
                report.blockers.push(blocker(
                    &format!("{}_install_copy_failed", file.kind.to_ascii_lowercase()),
                    format!(
                        "Failed to install the {} rebuilt output into {}: {error}",
                        file.kind,
                        target_path.display()
                    ),
                ));
                break;
            }
            installed_any_file = true;

            let built_sha256 = match file.built_sha256.clone() {
                Some(value) => value,
                None => match hash_path(&built_path) {
                    Ok(value) => value,
                    Err(error) => {
                        report.blockers.push(blocker(
                            &format!("{}_built_hash_failed", file.kind.to_ascii_lowercase()),
                            format!(
                                "Failed to hash the {} built output at {} after install copy: {error}",
                                file.kind,
                                built_path.display()
                            ),
                        ));
                        break;
                    }
                },
            };
            let installed_sha256 = match hash_path(&target_path) {
                Ok(value) => value,
                Err(error) => {
                    report.blockers.push(blocker(
                        &format!("{}_installed_hash_failed", file.kind.to_ascii_lowercase()),
                        format!(
                            "Failed to hash the installed {} destination file at {}: {error}",
                            file.kind,
                            target_path.display()
                        ),
                    ));
                    break;
                }
            };
            if installed_sha256 != built_sha256 {
                report.blockers.push(blocker(
                    &format!("{}_installed_hash_mismatch", file.kind.to_ascii_lowercase()),
                    format!(
                        "Installed {} hash mismatch at {}.",
                        file.kind,
                        target_path.display()
                    ),
                ));
                break;
            }

            file.built_sha256 = Some(built_sha256);
            file.installed_sha256 = Some(installed_sha256);
        }

        if !report.blockers.is_empty() {
            report.installed = installed_any_file;
            report.status = if installed_any_file {
                "installed_with_errors".to_string()
            } else {
                "blocked".to_string()
            };
            return Ok(report);
        }

        let installed_at = Utc::now();
        report.installed = true;
        report.installed_at = Some(installed_at);
        report.status = "installed".to_string();

        let install_manifest_path = PathBuf::from(&report.profile_backup_manifest_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| {
                PathBuf::from(&context.preview.workspace_root)
                    .join("backups")
                    .join(&context.preview.profile_name)
            })
            .join(INSTALL_MANIFEST_FILENAME);
        report.install_manifest_path = Some(install_manifest_path.display().to_string());

        let mut plan = context.plan.clone();
        plan.install_status = "installed".to_string();
        plan.last_install = Some(report.clone());
        if context.plan_exists_in_db {
            if let Err(error) = self.persist_install_metadata(&plan, &report, &cooked_root) {
                report.warnings.push(warning(
                    "install_metadata_persist_failed",
                    format!(
                        "Install completed but SQLite install metadata could not be persisted: {error}"
                    ),
                ));
                report.status = "installed_with_warnings".to_string();
            }
        } else {
            report.warnings.push(warning(
                "install_metadata_plan_not_persisted",
                "Plan is not present in the SQLite swap_plans table; install metadata was written to the plan JSON only.",
            ));
            report.status = "installed_with_warnings".to_string();
        }

        if let Err(error) = save_json(&install_manifest_path, &report) {
            report.blockers.push(blocker(
                "install_manifest_write_failed",
                format!(
                    "Install completed but the install manifest could not be written to {}: {error}",
                    install_manifest_path.display()
                ),
            ));
            report.status = "installed_with_errors".to_string();
        }

        plan.last_install = Some(report.clone());
        if let Err(error) = write_plan_file(&request.plan_path, &plan) {
            report.blockers.push(blocker(
                "install_plan_write_failed",
                format!(
                    "Install completed but the plan JSON could not be updated at {}: {error}",
                    request.plan_path.display()
                ),
            ));
            report.status = "installed_with_errors".to_string();
            return Ok(report);
        }
        if context.plan_exists_in_db {
            let _ = self.update_plan_notes(&plan);
        }

        Ok(report)
    }

    fn build_preview_context(
        &self,
        request: &InstallPreviewRequest,
    ) -> Result<InstallPreviewContext> {
        let plan = read_plan_file(&request.plan_path)?;
        let build_report = request
            .build_report
            .clone()
            .or_else(|| plan.last_build.clone());
        let configured_cooked_root = resolve_configured_cooked_root(&self.database, request)?;
        let workspace_root = request
            .workspace_root
            .clone()
            .unwrap_or_else(|| self.database.app_home().join("workspace"));
        let build_root = build_report
            .as_ref()
            .map(|report| PathBuf::from(&report.build_root))
            .unwrap_or_else(|| workspace_root.join("builds").join(&plan.profile_name));
        let original_backup_root = workspace_root.join("original_files_backup");
        let original_backup_manifest_path = original_backup_root.join("manifest.json");
        let manifest = load_original_backup_manifest(&original_backup_manifest_path);

        let mut warnings = Vec::new();
        let mut blockers = Vec::new();
        let mut files = Vec::new();
        let mut profile_backups = Vec::new();
        let mut permanent_original_backups = Vec::new();

        if plan.schema_version != SUPPORTED_PLAN_SCHEMA_VERSION {
            blockers.push(blocker(
                "unsupported_plan_schema",
                format!(
                    "Unsupported plan schema_version {}. This installer preview only supports schema_version {}.",
                    plan.schema_version, SUPPORTED_PLAN_SCHEMA_VERSION
                ),
            ));
        }

        let Some(build_report) = build_report else {
            blockers.push(blocker(
                "missing_build_report",
                "The saved plan does not contain a build report yet. Run 'bakkeswap build --plan <plan_path>' first.".to_string(),
            ));
            let preview = finalize_preview(
                &plan,
                &configured_cooked_root,
                &workspace_root,
                &build_root,
                &original_backup_manifest_path,
                files,
                profile_backups,
                permanent_original_backups,
                warnings,
                blockers,
            );
            return Ok(InstallPreviewContext {
                plan_exists_in_db: self.plan_exists_in_database(&plan.plan_id).unwrap_or(false),
                plan,
                preview,
            });
        };

        if plan.status != "built" || build_report.status != "built" {
            blockers.push(blocker(
                "plan_not_built",
                format!(
                    "The saved plan is not in a successful build state (plan status={}, build status={}). Run 'bakkeswap build --plan <plan_path>' first.",
                    plan.status, build_report.status
                ),
            ));
        }
        if !build_report.blockers.is_empty() {
            blockers.push(blocker(
                "build_report_blocked",
                "The saved build report contains blockers. Rebuild the plan before attempting an install preview.".to_string(),
            ));
        }
        if !configured_cooked_root.exists() {
            blockers.push(blocker(
                "configured_cooked_root_missing",
                format!(
                    "Configured CookedPCConsole directory does not exist: {}",
                    configured_cooked_root.display()
                ),
            ));
        } else if !configured_cooked_root.is_dir() {
            blockers.push(blocker(
                "configured_cooked_root_not_directory",
                format!(
                    "Configured CookedPCConsole path is not a directory: {}",
                    configured_cooked_root.display()
                ),
            ));
        } else if !is_cookedpcconsole_path(&configured_cooked_root) {
            blockers.push(blocker(
                "configured_cooked_root_invalid",
                format!(
                    "Configured cooked root must point to a CookedPCConsole directory: {}",
                    configured_cooked_root.display()
                ),
            ));
        }
        if let Some(plan_cooked_root) = plan.configured_cooked_root.as_deref() {
            if normalize_compare_path(plan_cooked_root)
                != normalize_compare_path_path(&configured_cooked_root)
            {
                warnings.push(warning(
                    "configured_cooked_root_mismatch",
                    "Configured CookedPCConsole path differs from the path used when this plan was created. Refresh the database and rebuild the plan if local package availability changed.",
                ));
            }
        }

        build_preview_for_operation(
            operation(&plan, "visual"),
            build_report.visual_output_path.as_deref(),
            build_report.visual_validation.as_ref(),
            true,
            &configured_cooked_root,
            &workspace_root,
            &original_backup_root,
            &manifest,
            &mut files,
            &mut profile_backups,
            &mut permanent_original_backups,
            &mut warnings,
            &mut blockers,
            &plan.profile_name,
        );
        build_preview_for_operation(
            operation(&plan, "thumbnail"),
            build_report.thumbnail_output_path.as_deref(),
            build_report.thumbnail_validation.as_ref(),
            false,
            &configured_cooked_root,
            &workspace_root,
            &original_backup_root,
            &manifest,
            &mut files,
            &mut profile_backups,
            &mut permanent_original_backups,
            &mut warnings,
            &mut blockers,
            &plan.profile_name,
        );

        let preview = finalize_preview(
            &plan,
            &configured_cooked_root,
            &workspace_root,
            &build_root,
            &original_backup_manifest_path,
            files,
            profile_backups,
            permanent_original_backups,
            warnings,
            blockers,
        );
        Ok(InstallPreviewContext {
            plan_exists_in_db: self.plan_exists_in_database(&plan.plan_id).unwrap_or(false),
            plan,
            preview,
        })
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

    fn persist_install_metadata(
        &self,
        plan: &SwapPlan,
        report: &InstallReport,
        cooked_root: &Path,
    ) -> Result<()> {
        let installed_at = report
            .installed_at
            .ok_or_else(|| anyhow!("install metadata persistence requires installed_at"))?;
        let install_manifest_path = report.install_manifest_path.as_deref();
        let install_id = install_id(&plan.plan_id, &installed_at.to_rfc3339());
        let files_json = serde_json::to_string(&report.files)?;
        let connection = self.database.connect()?;

        connection.execute(
            "UPDATE installed_swaps SET active = 0 WHERE profile_name = ?1 AND active = 1",
            params![report.profile_name],
        )?;
        connection.execute(
            "INSERT INTO installed_swaps (
                install_id, plan_id, profile_name, cooked_root, manifest_path,
                installed_at, restored_at, active, dry_run_only, files_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 1, 0, ?7)",
            params![
                install_id,
                plan.plan_id,
                report.profile_name,
                cooked_root.display().to_string(),
                install_manifest_path,
                installed_at.to_rfc3339(),
                files_json,
            ],
        )?;
        connection.execute(
            "UPDATE swap_plans
             SET notes_json = ?2
             WHERE plan_id = ?1",
            params![plan.plan_id, plan_notes_json(plan)?],
        )?;

        Ok(())
    }

    fn update_plan_notes(&self, plan: &SwapPlan) -> Result<()> {
        let connection = self.database.connect()?;
        connection.execute(
            "UPDATE swap_plans
             SET notes_json = ?2
             WHERE plan_id = ?1",
            params![plan.plan_id, plan_notes_json(plan)?],
        )?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_preview_for_operation(
    operation: Option<&SwapOperation>,
    built_output_path: Option<&str>,
    stored_validation: Option<&crate::upk::SandboxRebuildValidationResult>,
    required: bool,
    configured_cooked_root: &Path,
    workspace_root: &Path,
    original_backup_root: &Path,
    manifest: &OriginalBackupManifest,
    files: &mut Vec<InstallPreviewFile>,
    profile_backups: &mut Vec<BackupPreview>,
    permanent_original_backups: &mut Vec<BackupPreview>,
    warnings: &mut Vec<InstallWarning>,
    blockers: &mut Vec<InstallBlocker>,
    profile_name: &str,
) {
    let Some(operation) = operation else {
        if required {
            blockers.push(blocker(
                "missing_visual_operation",
                "The saved plan does not define a visual operation.".to_string(),
            ));
        }
        return;
    };

    if !operation.enabled {
        if required {
            blockers.push(blocker(
                "visual_operation_disabled",
                "The saved plan does not have an enabled visual operation to preview.".to_string(),
            ));
        }
        return;
    }

    let Some(target_filename) = operation.target_filename.as_deref() else {
        blockers.push(blocker(
            &format!(
                "{}_target_filename_missing",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} operation is enabled but the target filename is missing from the plan.",
                operation.kind
            ),
        ));
        return;
    };

    let relative_target_path = match safe_relative_path(target_filename) {
        Ok(path) => path,
        Err(error) => {
            blockers.push(blocker(
                &format!(
                    "{}_target_filename_invalid",
                    operation.kind.to_ascii_lowercase()
                ),
                format!(
                    "The {} operation defines an unsafe target filename '{}': {error}",
                    operation.kind, target_filename
                ),
            ));
            return;
        }
    };

    let Some(built_output_path) = built_output_path else {
        blockers.push(blocker(
            &format!(
                "{}_built_output_missing",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The saved build report does not include a {} output path.",
                operation.kind
            ),
        ));
        return;
    };

    let built_path = PathBuf::from(built_output_path);
    let target_path = configured_cooked_root.join(&relative_target_path);
    let profile_backup_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join(&relative_target_path);
    let relative_path = normalize_relative_path(&relative_target_path);
    let original_backup_path = original_backup_root.join(&relative_target_path);

    if !path_is_within_root(configured_cooked_root, &target_path) {
        blockers.push(blocker(
            &format!(
                "{}_destination_outside_cooked_root",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} destination path escapes the configured CookedPCConsole root: {}",
                operation.kind,
                target_path.display()
            ),
        ));
        return;
    }
    if path_is_within_root(configured_cooked_root, &built_path) {
        blockers.push(blocker(
            &format!(
                "{}_built_output_inside_cooked_root",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} built output must stay outside CookedPCConsole but resolved to {}.",
                operation.kind,
                built_path.display()
            ),
        ));
        return;
    }
    if let Some(source_path) = operation.source_path.as_deref().map(PathBuf::from) {
        if paths_collide(&source_path, &target_path) {
            blockers.push(blocker(
                &format!(
                    "{}_source_target_collision",
                    operation.kind.to_ascii_lowercase()
                ),
                format!(
                    "The {} source path collides with the install destination: {}",
                    operation.kind,
                    source_path.display()
                ),
            ));
            return;
        }
        if paths_collide(&source_path, &built_path) {
            blockers.push(blocker(
                &format!(
                    "{}_source_build_collision",
                    operation.kind.to_ascii_lowercase()
                ),
                format!(
                    "The {} source path collides with the built output path: {}",
                    operation.kind,
                    source_path.display()
                ),
            ));
            return;
        }
    }
    if paths_collide(&target_path, &built_path) {
        blockers.push(blocker(
            &format!(
                "{}_target_build_collision",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} install destination collides with the built output path: {}",
                operation.kind,
                target_path.display()
            ),
        ));
        return;
    }

    let built_exists = built_path.exists();
    if !built_exists {
        blockers.push(blocker(
            &format!(
                "{}_built_output_missing",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} built output does not exist on disk: {}",
                operation.kind,
                built_path.display()
            ),
        ));
    }
    let destination_exists = target_path.exists();
    if !destination_exists {
        blockers.push(blocker(
            &format!(
                "{}_destination_missing",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The {} destination file does not exist in CookedPCConsole: {}",
                operation.kind,
                target_path.display()
            ),
        ));
    }

    let built_output_sha256 = hash_path(&built_path).ok();
    let current_destination_sha256 = hash_path(&target_path).ok();
    let planned_original_sha256 = operation.target_sha256.clone();

    if let Some(validation) = stored_validation {
        if !validation.passed {
            blockers.push(blocker(
                &format!("{}_stored_validation_failed", operation.kind.to_ascii_lowercase()),
                format!(
                    "The stored {} build validation did not pass. Rebuild the plan before previewing install.",
                    operation.kind
                ),
            ));
        }
        if let (Some(expected_sha), Some(actual_sha)) = (
            validation.output_sha256.as_deref(),
            built_output_sha256.as_deref(),
        ) {
            if expected_sha != actual_sha {
                blockers.push(blocker(
                    &format!("{}_built_output_drift", operation.kind.to_ascii_lowercase()),
                    format!(
                        "The {} built output hash no longer matches the stored build validation for {}.",
                        operation.kind,
                        built_path.display()
                    ),
                ));
            }
        } else {
            warnings.push(warning(
                &format!("{}_stored_validation_incomplete", operation.kind.to_ascii_lowercase()),
                format!(
                    "The {} build validation did not record an output hash, so output drift could not be verified.",
                    operation.kind
                ),
            ));
        }
    } else {
        warnings.push(warning(
            &format!(
                "{}_stored_validation_missing",
                operation.kind.to_ascii_lowercase()
            ),
            format!(
                "The saved build report does not include stored validation for the {} output.",
                operation.kind
            ),
        ));
    }

    let current_matches_planned_original = current_destination_sha256
        .as_deref()
        .zip(planned_original_sha256.as_deref())
        .map(|(left, right)| left == right);
    let current_matches_built_output = current_destination_sha256
        .as_deref()
        .zip(built_output_sha256.as_deref())
        .map(|(left, right)| left == right);

    if let Some(false) = current_matches_planned_original {
        warnings.push(warning(
            &format!("{}_destination_hash_drift", operation.kind.to_ascii_lowercase()),
            format!(
                "The current {} destination hash differs from the original target hash recorded in the plan for {}.",
                operation.kind,
                target_path.display()
            ),
        ));
    }

    files.push(InstallPreviewFile {
        kind: operation.kind.clone(),
        target_filename: target_filename.to_string(),
        target_path: target_path.display().to_string(),
        built_path: built_path.display().to_string(),
        destination_exists,
        built_exists,
        would_overwrite: destination_exists,
        current_destination_sha256: current_destination_sha256.clone(),
        planned_original_sha256: planned_original_sha256.clone(),
        built_output_sha256: built_output_sha256.clone(),
        current_matches_planned_original,
        current_matches_built_output,
    });

    let profile_backup_exists = profile_backup_path.exists();
    let profile_backup_status = if profile_backup_exists {
        "existing backup reused".to_string()
    } else {
        "new backup will be created".to_string()
    };
    profile_backups.push(BackupPreview {
        backup_kind: "profile".to_string(),
        operation_kind: operation.kind.clone(),
        target_relative_path: relative_path.clone(),
        backup_path: profile_backup_path.display().to_string(),
        exists: profile_backup_exists,
        would_create: !profile_backup_exists,
        status: profile_backup_status,
        warnings: Vec::new(),
    });

    let manifest_record = manifest.files.get(&relative_path);
    let original_backup_exists = original_backup_path.exists();
    let mut original_backup_warnings = Vec::new();
    let original_backup_status = match (manifest_record, original_backup_exists) {
        (None, false) => "new permanent original backup will be created".to_string(),
        (Some(_), false) => {
            original_backup_warnings.push(
                "Manifest entry exists but the permanent original backup file is missing."
                    .to_string(),
            );
            "permanent original backup requires attention (missing-file)".to_string()
        }
        (None, true) => {
            original_backup_warnings.push(
                "A permanent original backup file exists but is missing from manifest.json."
                    .to_string(),
            );
            "permanent original backup requires attention (untracked-file)".to_string()
        }
        (Some(record), true) => {
            let backup_sha = hash_path(&original_backup_path).ok();
            if let Some(backup_sha) = backup_sha.as_deref() {
                let manifest_sha = record.sha256.as_str();
                if backup_sha != manifest_sha {
                    original_backup_warnings.push(
                        "Permanent original backup hash does not match manifest.json.".to_string(),
                    );
                    "permanent original backup requires attention (hash-mismatch)".to_string()
                } else {
                    "existing permanent original backup verified".to_string()
                }
            } else {
                "existing permanent original backup verified".to_string()
            }
        }
    };
    for message in &original_backup_warnings {
        warnings.push(warning(
            &format!(
                "{}_original_backup_warning",
                operation.kind.to_ascii_lowercase()
            ),
            format!("{} backup warning: {}", operation.kind, message),
        ));
    }
    permanent_original_backups.push(BackupPreview {
        backup_kind: "permanent_original".to_string(),
        operation_kind: operation.kind.clone(),
        target_relative_path: relative_path,
        backup_path: original_backup_path.display().to_string(),
        exists: original_backup_exists,
        would_create: !original_backup_exists && manifest_record.is_none(),
        status: original_backup_status,
        warnings: original_backup_warnings,
    });
}

#[allow(clippy::too_many_arguments)]
fn finalize_preview(
    plan: &SwapPlan,
    configured_cooked_root: &Path,
    workspace_root: &Path,
    build_root: &Path,
    original_backup_manifest_path: &Path,
    files: Vec<InstallPreviewFile>,
    profile_backups: Vec<BackupPreview>,
    permanent_original_backups: Vec<BackupPreview>,
    warnings: Vec<InstallWarning>,
    blockers: Vec<InstallBlocker>,
) -> InstallPreview {
    InstallPreview {
        plan_path: plan.plan_path.clone(),
        profile_name: plan.profile_name.clone(),
        status: if blockers.is_empty() {
            "preview_ready".to_string()
        } else {
            "blocked".to_string()
        },
        configured_cooked_root: configured_cooked_root.display().to_string(),
        workspace_root: workspace_root.display().to_string(),
        build_root: build_root.display().to_string(),
        files,
        profile_backups,
        permanent_original_backups,
        original_backup_manifest_path: original_backup_manifest_path.display().to_string(),
        restore_command: format!("bakkeswap restore --profile {}", plan.profile_name),
        confirmation_phrase: format!("INSTALL {}", plan.profile_name),
        dry_run_only: true,
        no_files_written: true,
        warnings,
        blockers,
    }
}

fn initialize_install_report(
    plan: &SwapPlan,
    preview: &InstallPreview,
    overwrite_profile_backup: bool,
) -> InstallReport {
    let files = preview
        .files
        .iter()
        .map(|file| {
            let profile_backup = find_backup_preview(&preview.profile_backups, &file.kind);
            let original_backup =
                find_backup_preview(&preview.permanent_original_backups, &file.kind);
            InstalledFileRecord {
                kind: file.kind.clone(),
                relative_path: profile_backup
                    .map(|value| value.target_relative_path.clone())
                    .unwrap_or_else(|| file.target_filename.clone()),
                target_path: file.target_path.clone(),
                built_path: file.built_path.clone(),
                profile_backup_path: profile_backup
                    .map(|value| value.backup_path.clone())
                    .unwrap_or_default(),
                original_backup_path: original_backup
                    .map(|value| value.backup_path.clone())
                    .unwrap_or_default(),
                original_sha256: file.planned_original_sha256.clone(),
                built_sha256: file.built_output_sha256.clone(),
                installed_sha256: None,
            }
        })
        .collect::<Vec<_>>();

    InstallReport {
        plan_id: plan.plan_id.clone(),
        plan_path: preview.plan_path.clone(),
        profile_name: preview.profile_name.clone(),
        status: "blocked".to_string(),
        installed: false,
        installed_at: None,
        restored_at: None,
        cooked_root: preview.configured_cooked_root.clone(),
        profile_backup_manifest_path: PathBuf::from(&preview.workspace_root)
            .join("backups")
            .join(&preview.profile_name)
            .join("manifest.json")
            .display()
            .to_string(),
        original_backup_manifest_path: preview.original_backup_manifest_path.clone(),
        install_manifest_path: None,
        overwrite_profile_backup,
        files,
        warnings: preview.warnings.clone(),
        blockers: Vec::new(),
        restore_command: preview.restore_command.clone(),
        confirmation_phrase: preview.confirmation_phrase.clone(),
    }
}

fn merge_backup_result(report: &mut InstallReport, result: &BackupResult, prefix: &str) {
    for warning_item in &result.warnings {
        report.warnings.push(warning(
            &format!("{}_{}", prefix, warning_item.code),
            warning_item.message.clone(),
        ));
    }
    for blocker_item in &result.blockers {
        report.blockers.push(blocker(
            &format!("{}_{}", prefix, blocker_item.code),
            blocker_item.message.clone(),
        ));
    }
}

fn validate_install_copy_paths(
    kind: &str,
    cooked_root: &Path,
    target_path: &Path,
    built_path: &Path,
) -> Option<InstallBlocker> {
    if !path_is_within_root(cooked_root, target_path) {
        return Some(blocker(
            &format!(
                "{}_destination_outside_cooked_root",
                kind.to_ascii_lowercase()
            ),
            format!(
                "The {} destination path escapes the configured CookedPCConsole root: {}",
                kind,
                target_path.display()
            ),
        ));
    }
    if path_is_within_root(cooked_root, built_path) {
        return Some(blocker(
            &format!("{}_built_output_inside_cooked_root", kind.to_ascii_lowercase()),
            format!(
                "The {} built output is inside CookedPCConsole and cannot be installed from there: {}",
                kind,
                built_path.display()
            ),
        ));
    }
    if paths_collide(target_path, built_path) {
        return Some(blocker(
            &format!("{}_target_build_collision", kind.to_ascii_lowercase()),
            format!(
                "The {} install destination collides with the built output path: {}",
                kind,
                target_path.display()
            ),
        ));
    }
    None
}

fn resolve_configured_cooked_root(
    database: &DatabaseService,
    request: &InstallPreviewRequest,
) -> Result<PathBuf> {
    if let Some(path) = &request.configured_cooked_root {
        return Ok(path.clone());
    }

    database
        .get_string_setting(COOKED_DIR_KEY)?
        .map(PathBuf::from)
        .ok_or_else(|| {
            anyhow!(
                "No configured CookedPCConsole path is available. Run 'bakkeswap config set-game-path <path>' first or pass an explicit cooked root to the installer preview service."
            )
        })
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
                "failed to create plan directory while updating install results at {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(plan)?;
    fs::write(plan_path, format!("{payload}\n")).with_context(|| {
        format!(
            "failed to write updated plan install results to {}",
            plan_path.display()
        )
    })?;
    Ok(())
}

fn operation<'a>(plan: &'a SwapPlan, kind: &str) -> Option<&'a SwapOperation> {
    plan.operations
        .iter()
        .find(|operation| operation.kind == kind)
}

fn find_backup_preview<'a>(previews: &'a [BackupPreview], kind: &str) -> Option<&'a BackupPreview> {
    previews
        .iter()
        .find(|preview| preview.operation_kind == kind)
}

fn load_original_backup_manifest(path: &Path) -> OriginalBackupManifest {
    let Ok(payload) = fs::read_to_string(path) else {
        return OriginalBackupManifest::default();
    };

    serde_json::from_str(&payload).unwrap_or_default()
}

fn copy_file(source_path: &Path, target_path: &Path) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::copy(source_path, target_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

fn hash_path(path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to open {} for hashing", path.display()))?;
    std::io::copy(&mut file, &mut digest)
        .with_context(|| format!("failed to hash {}", path.display()))?;
    Ok(format!("{:x}", digest.finalize()))
}

fn save_json<T>(path: &Path, payload: &T) -> Result<()>
where
    T: serde::Serialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let encoded = serde_json::to_string_pretty(payload)?;
    fs::write(path, format!("{encoded}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn is_cookedpcconsole_path(path: &Path) -> bool {
    path.file_name()
        .map(|value| {
            value
                .to_string_lossy()
                .eq_ignore_ascii_case("CookedPCConsole")
        })
        .unwrap_or(false)
}

fn normalize_compare_path(value: &str) -> String {
    normalize_compare_path_path(Path::new(value))
}

fn normalize_compare_path_path(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::Prefix(value) => normalized.push(value.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
        }
    }

    normalized
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
    let root_value = normalize_compare_path_path(root);
    let candidate_value = normalize_compare_path_path(candidate);
    candidate_value == root_value || candidate_value.starts_with(&(root_value + "/"))
}

fn paths_collide(left: &Path, right: &Path) -> bool {
    normalize_compare_path_path(left) == normalize_compare_path_path(right)
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() {
        bail!("absolute paths are not allowed")
    }

    let mut output = PathBuf::new();
    let mut has_normal_component = false;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => {
                output.push(value);
                has_normal_component = true;
            }
            Component::ParentDir => bail!("parent-directory traversal is not allowed"),
            Component::Prefix(_) | Component::RootDir => bail!("rooted paths are not allowed"),
        }
    }

    if !has_normal_component {
        bail!("empty target filename is not allowed")
    }

    Ok(output)
}

fn plan_notes_json(plan: &SwapPlan) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "status": plan.status,
        "install_status": plan.install_status,
        "warnings": plan.warnings,
        "build_blockers": plan.build_blockers,
        "compatibility": plan.compatibility,
        "operations": plan.operations,
        "database_path": plan.database_path,
        "configured_cooked_root": plan.configured_cooked_root,
        "rollback_notes": plan.rollback_notes,
        "last_build": plan.last_build,
        "last_install": plan.last_install,
    }))?)
}

fn install_id(plan_id: &str, installed_at: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(plan_id.as_bytes());
    digest.update(b"::");
    digest.update(installed_at.as_bytes());
    let digest = format!("{:x}", digest.finalize());
    format!("install_{}", &digest[..16])
}

fn warning(code: &str, message: impl Into<String>) -> InstallWarning {
    InstallWarning {
        code: code.to_string(),
        message: message.into(),
    }
}

fn blocker(code: &str, message: String) -> InstallBlocker {
    InstallBlocker {
        code: code.to_string(),
        message,
    }
}
