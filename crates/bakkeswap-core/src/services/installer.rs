use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::database::DatabaseService;
use crate::domain::models::{
    BackupPreview, InstallBlocker, InstallPreview, InstallPreviewFile, InstallWarning,
    PlanBuildReport, SwapOperation, SwapPlan,
};

const COOKED_DIR_KEY: &str = "cooked_dir";
const SUPPORTED_PLAN_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallPreviewRequest {
    pub plan_path: PathBuf,
    pub build_report: Option<PlanBuildReport>,
    pub configured_cooked_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct InstallerService {
    database: DatabaseService,
}

#[derive(Debug, Default, Deserialize)]
struct OriginalBackupManifest {
    #[serde(default)]
    files: BTreeMap<String, OriginalBackupManifestEntry>,
}

#[derive(Debug, Default, Deserialize)]
struct OriginalBackupManifestEntry {
    #[serde(default)]
    sha256: Option<String>,
}

impl InstallerService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn preview_install(&self, request: &InstallPreviewRequest) -> Result<InstallPreview> {
        if !request.dry_run {
            bail!("Real install is not implemented yet. Use --dry-run.");
        }

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
            return Ok(finalize_preview(
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
            ));
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
                != normalize_compare_path(&configured_cooked_root.display().to_string())
            {
                warnings.push(warning(
                    "configured_cooked_root_mismatch",
                    "Configured CookedPCConsole path differs from the path used when this plan was created. Refresh the database and rebuild the plan if local package availability changed.",
                ));
            }
        }

        let visual_operation = operation(&plan, "visual");
        let thumbnail_operation = operation(&plan, "thumbnail");

        build_preview_for_operation(
            visual_operation,
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
            thumbnail_operation,
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

        Ok(finalize_preview(
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
        ))
    }

    pub fn install(&self, request: &InstallPreviewRequest) -> Result<InstallPreview> {
        if !request.dry_run {
            bail!("Real install is not implemented yet. Use --dry-run.");
        }

        self.preview_install(request)
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
    let target_path = configured_cooked_root.join(target_filename);
    let profile_backup_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join(target_filename);
    let relative_path = target_filename.replace('\\', "/");
    let original_backup_path = original_backup_root.join(target_filename);

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
            if let (Some(backup_sha), Some(manifest_sha)) =
                (backup_sha.as_deref(), record.sha256.as_deref())
            {
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

fn operation<'a>(plan: &'a SwapPlan, kind: &str) -> Option<&'a SwapOperation> {
    plan.operations
        .iter()
        .find(|operation| operation.kind == kind)
}

fn load_original_backup_manifest(path: &Path) -> OriginalBackupManifest {
    let Ok(payload) = fs::read_to_string(path) else {
        return OriginalBackupManifest::default();
    };

    serde_json::from_str(&payload).unwrap_or_default()
}

fn hash_path(path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to open {} for hashing", path.display()))?;
    std::io::copy(&mut file, &mut digest)
        .with_context(|| format!("failed to hash {}", path.display()))?;
    Ok(format!("{:x}", digest.finalize()))
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
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
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
