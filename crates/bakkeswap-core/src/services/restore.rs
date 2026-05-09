use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::database::DatabaseService;
use crate::domain::models::{
    InstallReport, OriginalBackupManifest, ProfileBackupManifest, RestoreBlocker,
    RestoreFileRecord, RestoreReport, RestoreWarning, SwapPlan,
};

const BACKUPS_DIR_NAME: &str = "backups";
const COOKED_DIR_KEY: &str = "cooked_dir";
const INSTALL_MANIFEST_FILENAME: &str = "install_manifest.json";
const MANIFEST_FILENAME: &str = "manifest.json";
const ORIGINALS_DIR_NAME: &str = "original_files_backup";
const ORIGINAL_BACKUP_KIND: &str = "permanent_original";
const PROFILE_BACKUP_KIND: &str = "profile";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RestorePreviewRequest {
    pub profile_name: String,
    pub from_originals: bool,
    pub configured_cooked_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RestoreExecutionRequest {
    pub profile_name: String,
    pub from_originals: bool,
    pub confirmation: Option<String>,
    pub configured_cooked_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RestoreService {
    database: DatabaseService,
}

struct RestoreContext {
    report: RestoreReport,
    cooked_root: PathBuf,
    install_manifest: Option<InstallReport>,
    install_manifest_path: PathBuf,
    plan: Option<SwapPlan>,
    plan_exists_in_db: bool,
}

impl RestoreService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn preview_restore(&self, request: &RestorePreviewRequest) -> Result<RestoreReport> {
        Ok(self.build_restore_context(request)?.report)
    }

    pub fn restore(&self, request: &RestoreExecutionRequest) -> Result<RestoreReport> {
        let preview_request = RestorePreviewRequest {
            profile_name: request.profile_name.clone(),
            from_originals: request.from_originals,
            configured_cooked_root: request.configured_cooked_root.clone(),
            workspace_root: request.workspace_root.clone(),
        };
        let mut context = self.build_restore_context(&preview_request)?;
        let mut report = context.report.clone();
        report.dry_run = false;

        if report.status != "preview_ready" || !report.blockers.is_empty() {
            report.status = "blocked".to_string();
            return Ok(report);
        }

        match request.confirmation.as_deref() {
            Some(value) if value.trim() == report.confirmation_phrase => {}
            Some(_) => {
                report.blockers.push(blocker(
                    "confirmation_mismatch",
                    format!(
                        "Confirmation phrase did not match. Type '{}' exactly to continue.",
                        report.confirmation_phrase
                    ),
                ));
                report.status = "blocked".to_string();
                return Ok(report);
            }
            None => {
                report.blockers.push(blocker(
                    "confirmation_required",
                    format!(
                        "Restore requires explicit confirmation. Pass --confirm '{}' to continue.",
                        report.confirmation_phrase
                    ),
                ));
                report.status = "blocked".to_string();
                return Ok(report);
            }
        }

        let mut restored_any_file = false;
        for file in &mut report.files {
            let source_path = PathBuf::from(&file.backup_source_path);
            let destination_path = PathBuf::from(&file.destination_path);
            if let Some(path_blocker) = validate_restore_copy_paths(
                &file.kind,
                &context.cooked_root,
                &source_path,
                &destination_path,
            ) {
                report.blockers.push(path_blocker);
                break;
            }

            if let Err(error) = copy_file(&source_path, &destination_path) {
                report.blockers.push(blocker(
                    &format!("{}_restore_copy_failed", file.kind.to_ascii_lowercase()),
                    format!(
                        "Failed to restore the {} file from {} to {}: {error}",
                        file.kind,
                        source_path.display(),
                        destination_path.display()
                    ),
                ));
                break;
            }
            restored_any_file = true;

            let restored_sha256 = match hash_path(&destination_path) {
                Ok(value) => value,
                Err(error) => {
                    report.blockers.push(blocker(
                        &format!("{}_restored_hash_failed", file.kind.to_ascii_lowercase()),
                        format!(
                            "Failed to hash the restored {} destination file at {}: {error}",
                            file.kind,
                            destination_path.display()
                        ),
                    ));
                    break;
                }
            };
            if restored_sha256 != file.expected_sha256 {
                report.blockers.push(blocker(
                    &format!("{}_restored_hash_mismatch", file.kind.to_ascii_lowercase()),
                    format!(
                        "Restored {} hash mismatch at {}.",
                        file.kind,
                        destination_path.display()
                    ),
                ));
                break;
            }

            file.actual_restored_sha256 = Some(restored_sha256);
        }

        if !report.blockers.is_empty() {
            report.restored = restored_any_file;
            report.status = if restored_any_file {
                "restored_with_errors".to_string()
            } else {
                "blocked".to_string()
            };
            return Ok(report);
        }

        let restored_at = Utc::now();
        report.restored = true;
        report.restored_at = Some(restored_at);

        if let Some(install_manifest) = context.install_manifest.as_mut() {
            install_manifest.restored_at = Some(restored_at);
            if let Err(error) = save_json(&context.install_manifest_path, install_manifest) {
                report.blockers.push(blocker(
                    "install_manifest_write_failed",
                    format!(
                        "Restore completed but the install manifest could not be updated at {}: {error}",
                        context.install_manifest_path.display()
                    ),
                ));
            }
        } else {
            report.warnings.push(warning(
                "install_manifest_missing",
                format!(
                    "Install manifest is missing at {}. Restore used the current configured cooked root instead.",
                    context.install_manifest_path.display()
                ),
            ));
        }

        if let Some(plan) = context.plan.as_mut() {
            plan.install_status = "restored".to_string();
            if let Some(last_install) = plan.last_install.as_mut() {
                last_install.restored_at = Some(restored_at);
            } else if let Some(install_manifest) = context.install_manifest.clone() {
                let mut last_install = install_manifest;
                last_install.restored_at = Some(restored_at);
                plan.last_install = Some(last_install);
            }

            if let Err(error) = write_plan_file(Path::new(&plan.plan_path), plan) {
                report.blockers.push(blocker(
                    "restore_plan_write_failed",
                    format!(
                        "Restore completed but the plan JSON could not be updated at {}: {error}",
                        plan.plan_path
                    ),
                ));
            } else if context.plan_exists_in_db {
                if let Err(error) = self.update_plan_notes(plan) {
                    report.warnings.push(warning(
                        "restore_plan_notes_update_failed",
                        format!(
                            "Restore updated the plan JSON but SQLite notes_json could not be refreshed: {error}"
                        ),
                    ));
                }
            }
        } else {
            report.warnings.push(warning(
                "restore_plan_unavailable",
                "Restore completed but no saved plan could be loaded for install status updates."
                    .to_string(),
            ));
        }

        if context.plan_exists_in_db {
            if let Some(plan_id) = report.plan_id.as_deref() {
                match self.persist_restore_metadata(plan_id, &report.profile_name, &restored_at) {
                Ok(0) => report.warnings.push(warning(
                    "restore_metadata_not_found",
                    format!(
                        "Restore completed but no active installed_swaps row was found for profile {} and plan {}.",
                        report.profile_name, plan_id
                    ),
                )),
                Ok(_) => {}
                Err(error) => report.warnings.push(warning(
                    "restore_metadata_persist_failed",
                    format!(
                        "Restore completed but SQLite restore metadata could not be persisted: {error}"
                    ),
                )),
            }
            }
        }

        finalize_restore_status(&mut report);
        Ok(report)
    }

    fn build_restore_context(&self, request: &RestorePreviewRequest) -> Result<RestoreContext> {
        let workspace_root = request
            .workspace_root
            .clone()
            .unwrap_or_else(|| self.database.app_home().join("workspace"));
        let profile_backup_root = workspace_root
            .join(BACKUPS_DIR_NAME)
            .join(&request.profile_name);
        let profile_manifest_path = profile_backup_root.join(MANIFEST_FILENAME);
        let install_manifest_path = profile_backup_root.join(INSTALL_MANIFEST_FILENAME);
        let originals_root = workspace_root.join(ORIGINALS_DIR_NAME);
        let originals_manifest_path = originals_root.join(MANIFEST_FILENAME);

        let mut warnings = Vec::new();
        let mut blockers = Vec::new();

        let install_manifest = load_install_manifest(&install_manifest_path, &mut blockers)?;
        let configured_cooked_root = resolve_restore_cooked_root(
            &self.database,
            request,
            install_manifest.as_ref(),
            &mut warnings,
            &mut blockers,
        )?;
        validate_cooked_root(&configured_cooked_root, &mut blockers);

        let mut plan = None;
        let mut plan_id = install_manifest.as_ref().map(|value| value.plan_id.clone());
        let mut plan_path = install_manifest
            .as_ref()
            .map(|value| value.plan_path.clone());

        if let Some(candidate_path) = plan_path.as_deref() {
            match load_plan(candidate_path) {
                Ok(Some(candidate_plan)) => {
                    plan_id = Some(candidate_plan.plan_id.clone());
                    plan_path = Some(candidate_plan.plan_path.clone());
                    plan = Some(candidate_plan);
                }
                Ok(None) => warnings.push(warning(
                    "restore_plan_missing",
                    format!("Saved plan path does not exist anymore: {}", candidate_path),
                )),
                Err(error) => warnings.push(warning(
                    "restore_plan_load_failed",
                    format!("Failed to load saved plan at {}: {error}", candidate_path),
                )),
            }
        }

        let mut files = Vec::new();
        if request.from_originals {
            warnings.push(warning(
                "emergency_original_restore_requested",
                "Emergency restore from permanent original backups was explicitly requested. This path should only be used when the per-profile backup is missing or invalid.".to_string(),
            ));
            let original_manifest = match load_original_backup_manifest(&originals_manifest_path) {
                Ok(value) => value,
                Err(error) => {
                    blockers.push(blocker(
                        "original_backup_manifest_missing",
                        format!(
                            "Permanent original backup manifest is not available at {}: {error}",
                            originals_manifest_path.display()
                        ),
                    ));
                    OriginalBackupManifest::default()
                }
            };

            for (relative_path, entry) in &original_manifest.files {
                build_restore_file_record(
                    kind_from_install_manifest(install_manifest.as_ref(), relative_path),
                    relative_path,
                    ORIGINAL_BACKUP_KIND,
                    &originals_root,
                    &configured_cooked_root,
                    &entry.sha256,
                    &mut files,
                    &mut blockers,
                );
            }
        } else {
            let profile_manifest = match load_profile_backup_manifest(&profile_manifest_path) {
                Ok(value) => value,
                Err(error) => {
                    blockers.push(blocker(
                        "profile_backup_manifest_missing",
                        format!(
                            "Profile backup manifest is not available at {}: {error}",
                            profile_manifest_path.display()
                        ),
                    ));
                    ProfileBackupManifest {
                        schema_version: 1,
                        profile_name: request.profile_name.clone(),
                        plan_path: String::new(),
                        created_at: Utc::now(),
                        overwritten_existing: false,
                        files: Default::default(),
                    }
                }
            };

            if plan.is_none() && !profile_manifest.plan_path.trim().is_empty() {
                match load_plan(&profile_manifest.plan_path) {
                    Ok(Some(candidate_plan)) => {
                        plan_id = Some(candidate_plan.plan_id.clone());
                        plan_path = Some(candidate_plan.plan_path.clone());
                        plan = Some(candidate_plan);
                    }
                    Ok(None) => warnings.push(warning(
                        "restore_plan_missing",
                        format!(
                            "Saved plan path from the profile backup manifest does not exist anymore: {}",
                            profile_manifest.plan_path
                        ),
                    )),
                    Err(error) => warnings.push(warning(
                        "restore_plan_load_failed",
                        format!(
                            "Failed to load the saved plan from the profile backup manifest at {}: {error}",
                            profile_manifest.plan_path
                        ),
                    )),
                }
            }

            for (relative_path, entry) in &profile_manifest.files {
                build_restore_file_record(
                    if entry.operation_kind.trim().is_empty() {
                        kind_from_install_manifest(install_manifest.as_ref(), relative_path)
                    } else {
                        entry.operation_kind.clone()
                    },
                    relative_path,
                    PROFILE_BACKUP_KIND,
                    &profile_backup_root,
                    &configured_cooked_root,
                    &entry.sha256,
                    &mut files,
                    &mut blockers,
                );
            }
        }

        if files.is_empty() {
            blockers.push(blocker(
                "restore_files_missing",
                format!(
                    "No restore files were found for profile {}.",
                    request.profile_name
                ),
            ));
        }

        let plan_exists_in_db = if let Some(plan_id) = plan_id.as_deref() {
            self.plan_exists_in_database(plan_id)?
        } else {
            false
        };

        let confirmation_phrase = if request.from_originals {
            format!("RESTORE ORIGINALS {}", request.profile_name)
        } else {
            format!("RESTORE {}", request.profile_name)
        };
        let restore_command = if request.from_originals {
            format!(
                "bakkeswap restore --profile {} --from-originals",
                request.profile_name
            )
        } else {
            format!("bakkeswap restore --profile {}", request.profile_name)
        };

        let report = RestoreReport {
            plan_id,
            plan_path,
            profile_name: request.profile_name.clone(),
            status: if blockers.is_empty() {
                "preview_ready".to_string()
            } else {
                "blocked".to_string()
            },
            dry_run: true,
            from_originals: request.from_originals,
            restored: false,
            restored_at: None,
            cooked_root: configured_cooked_root.display().to_string(),
            install_manifest_path: install_manifest
                .as_ref()
                .map(|_| install_manifest_path.display().to_string()),
            profile_backup_manifest_path: profile_manifest_path.display().to_string(),
            original_backup_manifest_path: originals_manifest_path.display().to_string(),
            files,
            warnings,
            blockers,
            restore_command,
            confirmation_phrase,
        };

        Ok(RestoreContext {
            report,
            cooked_root: configured_cooked_root,
            install_manifest,
            install_manifest_path,
            plan,
            plan_exists_in_db,
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

    fn persist_restore_metadata(
        &self,
        plan_id: &str,
        profile_name: &str,
        restored_at: &DateTime<Utc>,
    ) -> Result<usize> {
        let connection = self.database.connect()?;
        let rows_updated = connection.execute(
            "UPDATE installed_swaps
             SET restored_at = ?3, active = 0
             WHERE profile_name = ?1 AND plan_id = ?2 AND active = 1",
            params![profile_name, plan_id, restored_at.to_rfc3339()],
        )?;
        Ok(rows_updated)
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
fn build_restore_file_record(
    kind: String,
    relative_path: &str,
    backup_kind: &str,
    backup_root: &Path,
    cooked_root: &Path,
    expected_sha256: &str,
    files: &mut Vec<RestoreFileRecord>,
    blockers: &mut Vec<RestoreBlocker>,
) {
    let relative_path_buf = match safe_relative_path(relative_path) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(blocker(
                &format!("{}_relative_path_invalid", backup_kind),
                format!(
                    "Restore path '{}' is unsafe for the {} backup set: {error}",
                    relative_path, backup_kind
                ),
            ));
            return;
        }
    };

    let backup_source_path = backup_root.join(&relative_path_buf);
    let destination_path = cooked_root.join(&relative_path_buf);
    if !path_is_within_root(backup_root, &backup_source_path) {
        blockers.push(blocker(
            &format!("{}_backup_path_outside_root", backup_kind),
            format!(
                "Restore backup path escapes its root for {}: {}",
                relative_path,
                backup_source_path.display()
            ),
        ));
        return;
    }
    if !path_is_within_root(cooked_root, &destination_path) {
        blockers.push(blocker(
            "restore_destination_outside_cooked_root",
            format!(
                "Restore destination path escapes the configured CookedPCConsole root: {}",
                destination_path.display()
            ),
        ));
        return;
    }

    let destination_exists = destination_path.exists();
    let (backup_sha256, backup_hash_matches_expected) = if !backup_source_path.exists() {
        blockers.push(blocker(
            &format!("{}_backup_missing_file", backup_kind),
            format!(
                "Restore backup file is missing for {} at {}.",
                relative_path,
                backup_source_path.display()
            ),
        ));
        (None, None)
    } else if !backup_source_path.is_file() {
        blockers.push(blocker(
            &format!("{}_backup_not_file", backup_kind),
            format!(
                "Restore backup path is not a file for {} at {}.",
                relative_path,
                backup_source_path.display()
            ),
        ));
        (None, None)
    } else {
        match hash_path(&backup_source_path) {
            Ok(value) => {
                let matches_expected = value == expected_sha256;
                if !matches_expected {
                    blockers.push(blocker(
                        &format!("{}_backup_hash_mismatch", backup_kind),
                        format!(
                            "Restore backup hash mismatch for {} at {}.",
                            relative_path,
                            backup_source_path.display()
                        ),
                    ));
                }
                (Some(value), Some(matches_expected))
            }
            Err(error) => {
                blockers.push(blocker(
                    &format!("{}_backup_hash_failed", backup_kind),
                    format!(
                        "Failed to hash restore backup {} at {}: {error}",
                        relative_path,
                        backup_source_path.display()
                    ),
                ));
                (None, None)
            }
        }
    };

    files.push(RestoreFileRecord {
        kind,
        relative_path: normalize_relative_path(&relative_path_buf),
        backup_kind: backup_kind.to_string(),
        backup_source_path: backup_source_path.display().to_string(),
        destination_path: destination_path.display().to_string(),
        destination_exists,
        expected_sha256: expected_sha256.to_string(),
        backup_sha256,
        backup_hash_matches_expected,
        actual_restored_sha256: None,
    });
}

fn validate_restore_copy_paths(
    kind: &str,
    cooked_root: &Path,
    source_path: &Path,
    destination_path: &Path,
) -> Option<RestoreBlocker> {
    if !path_is_within_root(cooked_root, destination_path) {
        return Some(blocker(
            &format!(
                "{}_destination_outside_cooked_root",
                kind.to_ascii_lowercase()
            ),
            format!(
                "Restore destination path escapes the configured CookedPCConsole root: {}",
                destination_path.display()
            ),
        ));
    }
    if paths_collide(source_path, destination_path) {
        return Some(blocker(
            &format!("{}_source_destination_collision", kind.to_ascii_lowercase()),
            format!(
                "Restore source and destination paths collide for {}: {}",
                kind,
                destination_path.display()
            ),
        ));
    }
    None
}

fn resolve_restore_cooked_root(
    database: &DatabaseService,
    request: &RestorePreviewRequest,
    install_manifest: Option<&InstallReport>,
    warnings: &mut Vec<RestoreWarning>,
    blockers: &mut Vec<RestoreBlocker>,
) -> Result<PathBuf> {
    if let Some(install_manifest) = install_manifest {
        if let Some(configured_root) = request.configured_cooked_root.as_ref() {
            if normalize_compare_path_path(configured_root)
                != normalize_compare_path(&install_manifest.cooked_root)
            {
                warnings.push(warning(
                    "configured_cooked_root_mismatch",
                    "Configured CookedPCConsole path differs from the cooked root recorded in the install manifest. Restore will use the install manifest cooked root.".to_string(),
                ));
            }
        } else if let Some(configured_root) = database.get_string_setting(COOKED_DIR_KEY)? {
            if normalize_compare_path(&configured_root)
                != normalize_compare_path(&install_manifest.cooked_root)
            {
                warnings.push(warning(
                    "configured_cooked_root_mismatch",
                    "Configured CookedPCConsole path differs from the cooked root recorded in the install manifest. Restore will use the install manifest cooked root.".to_string(),
                ));
            }
        }
        return Ok(PathBuf::from(&install_manifest.cooked_root));
    }

    if let Some(path) = request.configured_cooked_root.as_ref() {
        return Ok(path.clone());
    }

    match database.get_string_setting(COOKED_DIR_KEY)? {
        Some(value) => Ok(PathBuf::from(value)),
        None => {
            blockers.push(blocker(
                "configured_cooked_root_missing",
                "No install manifest or configured CookedPCConsole path is available for restore. Run 'bakkeswap config set-game-path <path>' first or restore from an install that wrote install_manifest.json.".to_string(),
            ));
            Ok(database
                .app_home()
                .join("workspace")
                .join("missing_cooked_root"))
        }
    }
}

fn validate_cooked_root(cooked_root: &Path, blockers: &mut Vec<RestoreBlocker>) {
    if !cooked_root.exists() {
        blockers.push(blocker(
            "configured_cooked_root_missing",
            format!(
                "Configured CookedPCConsole directory does not exist: {}",
                cooked_root.display()
            ),
        ));
    } else if !cooked_root.is_dir() {
        blockers.push(blocker(
            "configured_cooked_root_not_directory",
            format!(
                "Configured CookedPCConsole path is not a directory: {}",
                cooked_root.display()
            ),
        ));
    } else if !is_cookedpcconsole_path(cooked_root) {
        blockers.push(blocker(
            "configured_cooked_root_invalid",
            format!(
                "Configured cooked root must point to a CookedPCConsole directory: {}",
                cooked_root.display()
            ),
        ));
    }
}

fn finalize_restore_status(report: &mut RestoreReport) {
    if !report.blockers.is_empty() {
        report.status = if report.restored {
            "restored_with_errors".to_string()
        } else {
            "blocked".to_string()
        };
        return;
    }

    report.status = if report.warnings.is_empty() {
        "restored".to_string()
    } else {
        "restored_with_warnings".to_string()
    };
    report.dry_run = false;
}

fn load_install_manifest(
    path: &Path,
    blockers: &mut Vec<RestoreBlocker>,
) -> Result<Option<InstallReport>> {
    if !path.exists() {
        return Ok(None);
    }

    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read install manifest from {}", path.display()))?;
    match serde_json::from_str(&payload) {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            blockers.push(blocker(
                "install_manifest_invalid",
                format!(
                    "Install manifest exists but could not be parsed at {}: {error}",
                    path.display()
                ),
            ));
            Ok(None)
        }
    }
}

fn load_plan(path: &str) -> Result<Option<SwapPlan>> {
    let plan_path = PathBuf::from(path);
    if !plan_path.exists() {
        return Ok(None);
    }

    let payload = fs::read_to_string(&plan_path)
        .with_context(|| format!("failed to read saved plan from {}", plan_path.display()))?;
    let mut plan: SwapPlan = serde_json::from_str(&payload)
        .with_context(|| format!("failed to parse saved plan from {}", plan_path.display()))?;
    plan.plan_path = plan_path.display().to_string();
    Ok(Some(plan))
}

fn write_plan_file(plan_path: &Path, plan: &SwapPlan) -> Result<()> {
    if let Some(parent) = plan_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create plan directory while updating restore results at {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(plan)?;
    fs::write(plan_path, format!("{payload}\n")).with_context(|| {
        format!(
            "failed to write updated plan restore results to {}",
            plan_path.display()
        )
    })?;
    Ok(())
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

fn kind_from_install_manifest(
    install_manifest: Option<&InstallReport>,
    relative_path: &str,
) -> String {
    install_manifest
        .and_then(|manifest| {
            manifest
                .files
                .iter()
                .find(|file| file.relative_path == relative_path)
                .map(|file| file.kind.clone())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn load_original_backup_manifest(path: &Path) -> Result<OriginalBackupManifest> {
    let payload = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read original backup manifest from {}",
            path.display()
        )
    })?;
    serde_json::from_str(&payload).with_context(|| {
        format!(
            "failed to parse original backup manifest from {}",
            path.display()
        )
    })
}

fn load_profile_backup_manifest(path: &Path) -> Result<ProfileBackupManifest> {
    let payload = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read profile backup manifest from {}",
            path.display()
        )
    })?;
    serde_json::from_str(&payload).with_context(|| {
        format!(
            "failed to parse profile backup manifest from {}",
            path.display()
        )
    })
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
        bail!("empty relative path is not allowed")
    }

    Ok(output)
}

fn warning(code: &str, message: String) -> RestoreWarning {
    RestoreWarning {
        code: code.to_string(),
        message,
    }
}

fn blocker(code: &str, message: String) -> RestoreBlocker {
    RestoreBlocker {
        code: code.to_string(),
        message,
    }
}
