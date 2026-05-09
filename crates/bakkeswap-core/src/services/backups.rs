use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::database::DatabaseService;
use crate::domain::models::{
    BackupBlocker, BackupFileResult, BackupPreview, BackupResult, BackupVerificationResult,
    BackupWarning, InstallPreview, OriginalBackupEntry, OriginalBackupManifest, ProfileBackupEntry,
    ProfileBackupManifest,
};

const BACKUPS_DIR_NAME: &str = "backups";
const ORIGINALS_DIR_NAME: &str = "original_files_backup";
const MANIFEST_FILENAME: &str = "manifest.json";
const ORIGINAL_BACKUP_KIND: &str = "permanent_original";
const PROFILE_BACKUP_KIND: &str = "profile";

#[derive(Debug, Clone)]
pub struct PermanentOriginalBackupManager {
    database: DatabaseService,
}

impl PermanentOriginalBackupManager {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn prepare_from_preview(&self, preview: &InstallPreview) -> Result<BackupResult> {
        let backup_root = original_backup_root_from_preview(preview);
        let manifest_path = PathBuf::from(&preview.original_backup_manifest_path);
        let mut result = empty_backup_result(
            ORIGINAL_BACKUP_KIND,
            Some(preview.profile_name.clone()),
            &backup_root,
            &manifest_path,
        );

        if block_if_preview_not_ready(preview, &mut result) {
            return Ok(result);
        }

        if let Some(blocker) =
            validate_manifest_path(&manifest_path, &backup_root, ORIGINAL_BACKUP_KIND)
        {
            result = empty_backup_result(
                ORIGINAL_BACKUP_KIND,
                Some(preview.profile_name.clone()),
                &backup_root,
                &manifest_path,
            );
            result.blockers.push(blocker);
            result.status = "blocked".to_string();
            return Ok(result);
        }

        let mut manifest = load_original_backup_manifest(&manifest_path)?;
        for file in &preview.files {
            let Some(backup_preview) =
                find_backup_preview(&preview.permanent_original_backups, &file.kind)
            else {
                result.blockers.push(blocker(
                    "missing_original_backup_preview",
                    format!(
                        "The install preview does not define a permanent original backup target for the {} operation.",
                        file.kind
                    ),
                ));
                continue;
            };

            if let Some(blocker) =
                validate_backup_preview_path(backup_preview, &backup_root, ORIGINAL_BACKUP_KIND)
            {
                result.blockers.push(blocker);
                continue;
            }

            let source_path = PathBuf::from(&file.target_path);
            let backup_path = PathBuf::from(&backup_preview.backup_path);
            let relative_path = backup_preview.target_relative_path.clone();
            let manifest_entry = manifest.files.get(&relative_path);
            let backup_exists = backup_path.exists();

            let source_sha256 =
                match hash_existing_source(&source_path, &file.kind, &mut result.blockers) {
                    Some(value) => value,
                    None => continue,
                };

            match (manifest_entry, backup_exists) {
                (None, false) => {
                    if let Err(error) = copy_file(&source_path, &backup_path) {
                        result.blockers.push(blocker(
                            "original_backup_copy_failed",
                            format!(
                                "Failed to create permanent original backup for {} at {}: {error}",
                                file.kind,
                                backup_path.display()
                            ),
                        ));
                        continue;
                    }

                    let Some((backup_sha256, size_bytes)) = verify_copied_backup(
                        &backup_path,
                        &source_sha256,
                        "original backup",
                        &file.kind,
                        &mut result,
                    ) else {
                        let _ = fs::remove_file(&backup_path);
                        continue;
                    };

                    manifest.files.insert(
                        relative_path.clone(),
                        OriginalBackupEntry {
                            target_relative_path: relative_path.clone(),
                            target_path: source_path.display().to_string(),
                            sha256: backup_sha256.clone(),
                            size_bytes,
                            created_at: Utc::now(),
                        },
                    );
                    if let Err(error) = save_json(&manifest_path, &manifest) {
                        result.blockers.push(blocker(
                            "original_backup_manifest_write_failed",
                            format!(
                                "Failed to write permanent original backup manifest to {}: {error}",
                                manifest_path.display()
                            ),
                        ));
                        continue;
                    }

                    result.created_count += 1;
                    result.verified_count += 1;
                    result.files.push(BackupFileResult {
                        backup_kind: ORIGINAL_BACKUP_KIND.to_string(),
                        operation_kind: file.kind.clone(),
                        target_relative_path: relative_path,
                        source_path: source_path.display().to_string(),
                        backup_path: backup_path.display().to_string(),
                        sha256: Some(backup_sha256),
                        size_bytes: Some(size_bytes),
                        status: "created".to_string(),
                        warnings: Vec::new(),
                    });
                }
                (Some(entry), true) => {
                    let backup_sha256 = match hash_path(&backup_path) {
                        Ok(value) => value,
                        Err(error) => {
                            result.blockers.push(blocker(
                                "original_backup_hash_failed",
                                format!(
                                    "Failed to verify permanent original backup for {} at {}: {error}",
                                    file.kind,
                                    backup_path.display()
                                ),
                            ));
                            continue;
                        }
                    };

                    if backup_sha256 != entry.sha256 {
                        result.blockers.push(blocker(
                            "original_backup_hash_mismatch",
                            format!(
                                "Permanent original backup hash mismatch for {} at {}.",
                                file.kind,
                                backup_path.display()
                            ),
                        ));
                        continue;
                    }

                    result.existing_count += 1;
                    result.verified_count += 1;
                    result.files.push(BackupFileResult {
                        backup_kind: ORIGINAL_BACKUP_KIND.to_string(),
                        operation_kind: file.kind.clone(),
                        target_relative_path: relative_path,
                        source_path: source_path.display().to_string(),
                        backup_path: backup_path.display().to_string(),
                        sha256: Some(backup_sha256),
                        size_bytes: Some(entry.size_bytes),
                        status: "already_exists".to_string(),
                        warnings: Vec::new(),
                    });
                }
                (Some(_), false) => {
                    result.blockers.push(blocker(
                        "original_backup_missing_file",
                        format!(
                            "Permanent original backup manifest entry exists but the backup file is missing for {}.",
                            file.kind
                        ),
                    ));
                }
                (None, true) => {
                    result.blockers.push(blocker(
                        "original_backup_untracked_file",
                        format!(
                            "A permanent original backup file already exists at {} but is missing from manifest.json.",
                            backup_path.display()
                        ),
                    ));
                }
            }
        }

        result.status = result_status(&result.blockers, "prepared");
        Ok(result)
    }

    pub fn status(&self) -> Result<BackupVerificationResult> {
        let backup_root = self
            .database
            .app_home()
            .join("workspace")
            .join(ORIGINALS_DIR_NAME);
        let manifest_path = backup_root.join(MANIFEST_FILENAME);
        inspect_original_backups(&backup_root, &manifest_path, false)
    }

    pub fn verify(&self) -> Result<BackupVerificationResult> {
        let backup_root = self
            .database
            .app_home()
            .join("workspace")
            .join(ORIGINALS_DIR_NAME);
        let manifest_path = backup_root.join(MANIFEST_FILENAME);
        inspect_original_backups(&backup_root, &manifest_path, true)
    }
}

#[derive(Debug, Clone)]
pub struct ProfileBackupManager {
    database: DatabaseService,
}

impl ProfileBackupManager {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn prepare_from_preview(
        &self,
        preview: &InstallPreview,
        overwrite_existing: bool,
    ) -> Result<BackupResult> {
        let backup_root = profile_backup_root_from_preview(preview);
        let manifest_path = backup_root.join(MANIFEST_FILENAME);
        let mut result = empty_backup_result(
            PROFILE_BACKUP_KIND,
            Some(preview.profile_name.clone()),
            &backup_root,
            &manifest_path,
        );

        let _ = &self.database;

        if block_if_preview_not_ready(preview, &mut result) {
            return Ok(result);
        }

        if let Some(blocker) =
            validate_manifest_path(&manifest_path, &backup_root, PROFILE_BACKUP_KIND)
        {
            result = empty_backup_result(
                PROFILE_BACKUP_KIND,
                Some(preview.profile_name.clone()),
                &backup_root,
                &manifest_path,
            );
            result.blockers.push(blocker);
            result.status = "blocked".to_string();
            return Ok(result);
        }

        let backup_root_exists = backup_root.exists();
        let backup_root_has_entries = if backup_root_exists {
            directory_has_entries(&backup_root)?
        } else {
            false
        };

        if backup_root_has_entries && !overwrite_existing {
            result.blockers.push(blocker(
                "profile_backup_already_exists",
                format!(
                    "Profile backup already exists at {}. Re-run with --overwrite-profile-backup to replace it explicitly.",
                    backup_root.display()
                ),
            ));
            result.status = "blocked".to_string();
            return Ok(result);
        }

        if backup_root_has_entries && overwrite_existing {
            if !manifest_path.exists() {
                result.blockers.push(blocker(
                    "profile_backup_missing_manifest",
                    format!(
                        "Profile backup directory {} already exists but manifest.json is missing. Refusing to overwrite it automatically.",
                        backup_root.display()
                    ),
                ));
                result.status = "blocked".to_string();
                return Ok(result);
            }

            let verification = inspect_profile_backups(&backup_root, &manifest_path, true)?;
            if !verification.blockers.is_empty() {
                result.blockers.extend(verification.blockers);
                result.warnings.extend(verification.warnings);
                result.status = "blocked".to_string();
                return Ok(result);
            }

            if let Err(error) = fs::remove_dir_all(&backup_root) {
                result.blockers.push(blocker(
                    "profile_backup_overwrite_cleanup_failed",
                    format!(
                        "Failed to remove existing profile backup at {} before overwrite: {error}",
                        backup_root.display()
                    ),
                ));
                result.status = "blocked".to_string();
                return Ok(result);
            }
        }

        fs::create_dir_all(&backup_root).with_context(|| {
            format!(
                "failed to create profile backup directory at {}",
                backup_root.display()
            )
        })?;

        let mut manifest = ProfileBackupManifest {
            schema_version: 1,
            profile_name: preview.profile_name.clone(),
            plan_path: preview.plan_path.clone(),
            created_at: Utc::now(),
            overwritten_existing: overwrite_existing,
            files: Default::default(),
        };

        for file in &preview.files {
            let Some(backup_preview) = find_backup_preview(&preview.profile_backups, &file.kind)
            else {
                result.blockers.push(blocker(
                    "missing_profile_backup_preview",
                    format!(
                        "The install preview does not define a profile backup target for the {} operation.",
                        file.kind
                    ),
                ));
                continue;
            };

            if let Some(blocker) =
                validate_backup_preview_path(backup_preview, &backup_root, PROFILE_BACKUP_KIND)
            {
                result.blockers.push(blocker);
                continue;
            }

            let source_path = PathBuf::from(&file.target_path);
            let backup_path = PathBuf::from(&backup_preview.backup_path);
            let relative_path = backup_preview.target_relative_path.clone();
            let source_sha256 =
                match hash_existing_source(&source_path, &file.kind, &mut result.blockers) {
                    Some(value) => value,
                    None => continue,
                };

            if let Err(error) = copy_file(&source_path, &backup_path) {
                result.blockers.push(blocker(
                    "profile_backup_copy_failed",
                    format!(
                        "Failed to create profile backup for {} at {}: {error}",
                        file.kind,
                        backup_path.display()
                    ),
                ));
                continue;
            }

            let Some((backup_sha256, size_bytes)) = verify_copied_backup(
                &backup_path,
                &source_sha256,
                "profile backup",
                &file.kind,
                &mut result,
            ) else {
                let _ = fs::remove_file(&backup_path);
                continue;
            };

            manifest.files.insert(
                relative_path.clone(),
                ProfileBackupEntry {
                    operation_kind: file.kind.clone(),
                    target_relative_path: relative_path.clone(),
                    target_path: source_path.display().to_string(),
                    sha256: backup_sha256.clone(),
                    size_bytes,
                    created_at: Utc::now(),
                },
            );
            if let Err(error) = save_json(&manifest_path, &manifest) {
                result.blockers.push(blocker(
                    "profile_backup_manifest_write_failed",
                    format!(
                        "Failed to write profile backup manifest to {}: {error}",
                        manifest_path.display()
                    ),
                ));
                continue;
            }

            result.created_count += 1;
            result.verified_count += 1;
            result.files.push(BackupFileResult {
                backup_kind: PROFILE_BACKUP_KIND.to_string(),
                operation_kind: file.kind.clone(),
                target_relative_path: relative_path,
                source_path: source_path.display().to_string(),
                backup_path: backup_path.display().to_string(),
                sha256: Some(backup_sha256),
                size_bytes: Some(size_bytes),
                status: if overwrite_existing {
                    "overwritten".to_string()
                } else {
                    "created".to_string()
                },
                warnings: Vec::new(),
            });
        }

        result.status = result_status(&result.blockers, "prepared");
        Ok(result)
    }

    pub fn restore_profile(&self, _profile_name: &str) -> Result<()> {
        bail!("not implemented: profile backup restore")
    }
}

fn inspect_original_backups(
    backup_root: &Path,
    manifest_path: &Path,
    verify_hash: bool,
) -> Result<BackupVerificationResult> {
    let manifest = load_original_backup_manifest(manifest_path)?;
    let mut result = empty_verification_result(ORIGINAL_BACKUP_KIND, backup_root, manifest_path);

    for (relative_path, entry) in &manifest.files {
        let backup_path = backup_root.join(relative_path);
        if !backup_path.exists() {
            result.missing_file_count += 1;
            result.blockers.push(blocker(
                "original_backup_missing_file",
                format!(
                    "Manifest entry exists but backup file is missing: {}",
                    backup_path.display()
                ),
            ));
            result.files.push(BackupFileResult {
                backup_kind: ORIGINAL_BACKUP_KIND.to_string(),
                operation_kind: "unknown".to_string(),
                target_relative_path: relative_path.clone(),
                source_path: entry.target_path.clone(),
                backup_path: backup_path.display().to_string(),
                sha256: None,
                size_bytes: None,
                status: "missing-file".to_string(),
                warnings: Vec::new(),
            });
            continue;
        }

        let mut file_warnings = Vec::new();
        let backup_sha256 = if verify_hash {
            match hash_path(&backup_path) {
                Ok(value) => Some(value),
                Err(error) => {
                    result.blockers.push(blocker(
                        "original_backup_hash_failed",
                        format!(
                            "Failed to verify original backup at {}: {error}",
                            backup_path.display()
                        ),
                    ));
                    None
                }
            }
        } else {
            None
        };

        let status = if let Some(backup_sha256) = backup_sha256.as_deref() {
            if backup_sha256 != entry.sha256 {
                result.mismatched_file_count += 1;
                result.blockers.push(blocker(
                    "original_backup_hash_mismatch",
                    format!(
                        "Original backup hash mismatch at {}.",
                        backup_path.display()
                    ),
                ));
                "hash-mismatch".to_string()
            } else {
                "verified".to_string()
            }
        } else {
            if !verify_hash {
                file_warnings
                    .push("Hash verification was not requested for status output.".to_string());
            }
            "ready".to_string()
        };

        result.files.push(BackupFileResult {
            backup_kind: ORIGINAL_BACKUP_KIND.to_string(),
            operation_kind: "unknown".to_string(),
            target_relative_path: relative_path.clone(),
            source_path: entry.target_path.clone(),
            backup_path: backup_path.display().to_string(),
            sha256: backup_sha256.or_else(|| Some(entry.sha256.clone())),
            size_bytes: Some(entry.size_bytes),
            status,
            warnings: file_warnings,
        });
    }

    let tracked_paths = manifest.files.keys().cloned().collect::<BTreeSet<_>>();
    for relative_path in collect_backup_relative_paths(backup_root)? {
        if tracked_paths.contains(&relative_path) {
            continue;
        }

        result.untracked_file_count += 1;
        result.warnings.push(warning(
            "original_backup_untracked_file",
            format!(
                "A permanent original backup file exists on disk but is missing from manifest.json: {}",
                relative_path
            ),
        ));
        result.files.push(BackupFileResult {
            backup_kind: ORIGINAL_BACKUP_KIND.to_string(),
            operation_kind: "unknown".to_string(),
            target_relative_path: relative_path.clone(),
            source_path: String::new(),
            backup_path: backup_root.join(&relative_path).display().to_string(),
            sha256: None,
            size_bytes: None,
            status: "untracked-file".to_string(),
            warnings: vec!["Present on disk but missing from manifest.json.".to_string()],
        });
    }

    result.tracked_file_count = manifest.files.len();
    result.status = if manifest.files.is_empty() && result.untracked_file_count == 0 {
        "empty".to_string()
    } else {
        result_status(&result.blockers, "ready")
    };
    Ok(result)
}

fn inspect_profile_backups(
    backup_root: &Path,
    manifest_path: &Path,
    verify_hash: bool,
) -> Result<BackupVerificationResult> {
    let manifest = load_profile_backup_manifest(manifest_path)?;
    let mut result = empty_verification_result(PROFILE_BACKUP_KIND, backup_root, manifest_path);

    for (relative_path, entry) in &manifest.files {
        let backup_path = backup_root.join(relative_path);
        if !backup_path.exists() {
            result.missing_file_count += 1;
            result.blockers.push(blocker(
                "profile_backup_missing_file",
                format!(
                    "Profile backup manifest entry exists but backup file is missing: {}",
                    backup_path.display()
                ),
            ));
            continue;
        }

        let backup_sha256 = if verify_hash {
            match hash_path(&backup_path) {
                Ok(value) => Some(value),
                Err(error) => {
                    result.blockers.push(blocker(
                        "profile_backup_hash_failed",
                        format!(
                            "Failed to verify profile backup at {}: {error}",
                            backup_path.display()
                        ),
                    ));
                    None
                }
            }
        } else {
            None
        };

        let status = if let Some(backup_sha256) = backup_sha256.as_deref() {
            if backup_sha256 != entry.sha256 {
                result.mismatched_file_count += 1;
                result.blockers.push(blocker(
                    "profile_backup_hash_mismatch",
                    format!("Profile backup hash mismatch at {}.", backup_path.display()),
                ));
                "hash-mismatch".to_string()
            } else {
                "verified".to_string()
            }
        } else {
            "ready".to_string()
        };

        result.files.push(BackupFileResult {
            backup_kind: PROFILE_BACKUP_KIND.to_string(),
            operation_kind: entry.operation_kind.clone(),
            target_relative_path: relative_path.clone(),
            source_path: entry.target_path.clone(),
            backup_path: backup_path.display().to_string(),
            sha256: backup_sha256.or_else(|| Some(entry.sha256.clone())),
            size_bytes: Some(entry.size_bytes),
            status,
            warnings: Vec::new(),
        });
    }

    let tracked_paths = manifest.files.keys().cloned().collect::<BTreeSet<_>>();
    for relative_path in collect_backup_relative_paths(backup_root)? {
        if tracked_paths.contains(&relative_path) {
            continue;
        }

        result.untracked_file_count += 1;
        result.warnings.push(warning(
            "profile_backup_untracked_file",
            format!(
                "A profile backup file exists on disk but is missing from manifest.json: {}",
                relative_path
            ),
        ));
    }

    result.tracked_file_count = manifest.files.len();
    result.status = result_status(&result.blockers, "ready");
    Ok(result)
}

fn original_backup_root_from_preview(preview: &InstallPreview) -> PathBuf {
    PathBuf::from(&preview.workspace_root).join(ORIGINALS_DIR_NAME)
}

fn profile_backup_root_from_preview(preview: &InstallPreview) -> PathBuf {
    PathBuf::from(&preview.workspace_root)
        .join(BACKUPS_DIR_NAME)
        .join(&preview.profile_name)
}

fn block_if_preview_not_ready(preview: &InstallPreview, result: &mut BackupResult) -> bool {
    if preview.status == "preview_ready" && preview.blockers.is_empty() {
        return false;
    }

    result.blockers.push(blocker(
        "install_preview_blocked",
        "Backup preparation requires an install preview in the preview_ready state.".to_string(),
    ));
    for preview_blocker in &preview.blockers {
        result.blockers.push(blocker(
            &format!("preview_{}", preview_blocker.code),
            preview_blocker.message.clone(),
        ));
    }
    result.status = "blocked".to_string();
    true
}

fn empty_backup_result(
    backup_kind: &str,
    profile_name: Option<String>,
    backup_root: &Path,
    manifest_path: &Path,
) -> BackupResult {
    BackupResult {
        backup_kind: backup_kind.to_string(),
        status: "prepared".to_string(),
        profile_name,
        backup_root: backup_root.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        files: Vec::new(),
        created_count: 0,
        existing_count: 0,
        verified_count: 0,
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

fn empty_verification_result(
    backup_kind: &str,
    backup_root: &Path,
    manifest_path: &Path,
) -> BackupVerificationResult {
    BackupVerificationResult {
        backup_kind: backup_kind.to_string(),
        status: "ready".to_string(),
        backup_root: backup_root.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        tracked_file_count: 0,
        missing_file_count: 0,
        mismatched_file_count: 0,
        untracked_file_count: 0,
        files: Vec::new(),
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

fn find_backup_preview<'a>(
    previews: &'a [BackupPreview],
    operation_kind: &str,
) -> Option<&'a BackupPreview> {
    previews
        .iter()
        .find(|preview| preview.operation_kind == operation_kind)
}

fn validate_manifest_path(
    manifest_path: &Path,
    backup_root: &Path,
    backup_kind: &str,
) -> Option<BackupBlocker> {
    if manifest_path.starts_with(backup_root) {
        return None;
    }

    Some(blocker(
        &format!("{}_manifest_outside_root", backup_kind),
        format!(
            "Backup manifest path must stay inside {} but resolved to {}.",
            backup_root.display(),
            manifest_path.display()
        ),
    ))
}

fn validate_backup_preview_path(
    preview: &BackupPreview,
    backup_root: &Path,
    backup_kind: &str,
) -> Option<BackupBlocker> {
    let backup_path = PathBuf::from(&preview.backup_path);
    if backup_path.starts_with(backup_root) {
        return None;
    }

    Some(blocker(
        &format!("{}_path_outside_root", backup_kind),
        format!(
            "Backup path for {} must stay inside {} but resolved to {}.",
            preview.operation_kind,
            backup_root.display(),
            backup_path.display()
        ),
    ))
}

fn hash_existing_source(
    source_path: &Path,
    operation_kind: &str,
    blockers: &mut Vec<BackupBlocker>,
) -> Option<String> {
    if !source_path.exists() {
        blockers.push(blocker(
            "backup_source_missing",
            format!(
                "Current destination file is missing for the {} operation: {}",
                operation_kind,
                source_path.display()
            ),
        ));
        return None;
    }

    if !source_path.is_file() {
        blockers.push(blocker(
            "backup_source_not_file",
            format!(
                "Current destination path for the {} operation is not a file: {}",
                operation_kind,
                source_path.display()
            ),
        ));
        return None;
    }

    match hash_path(source_path) {
        Ok(value) => Some(value),
        Err(error) => {
            blockers.push(blocker(
                "backup_source_hash_failed",
                format!(
                    "Failed to hash current destination file for the {} operation at {}: {error}",
                    operation_kind,
                    source_path.display()
                ),
            ));
            None
        }
    }
}

fn verify_copied_backup(
    backup_path: &Path,
    expected_sha256: &str,
    label: &str,
    operation_kind: &str,
    result: &mut BackupResult,
) -> Option<(String, u64)> {
    let backup_sha256 = match hash_path(backup_path) {
        Ok(value) => value,
        Err(error) => {
            result.blockers.push(blocker(
                "backup_hash_failed",
                format!(
                    "Failed to verify copied {} for {} at {}: {error}",
                    label,
                    operation_kind,
                    backup_path.display()
                ),
            ));
            return None;
        }
    };

    if backup_sha256 != expected_sha256 {
        result.blockers.push(blocker(
            "backup_hash_mismatch",
            format!(
                "Copied {} hash mismatch for {} at {}.",
                label,
                operation_kind,
                backup_path.display()
            ),
        ));
        return None;
    }

    let size_bytes = match fs::metadata(backup_path) {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            result.blockers.push(blocker(
                "backup_metadata_failed",
                format!(
                    "Failed to read metadata for {} at {}: {error}",
                    label,
                    backup_path.display()
                ),
            ));
            return None;
        }
    };

    Some((backup_sha256, size_bytes))
}

fn load_original_backup_manifest(path: &Path) -> Result<OriginalBackupManifest> {
    if !path.exists() {
        return Ok(OriginalBackupManifest::default());
    }

    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read original backup manifest {}", path.display()))?;
    serde_json::from_str(&payload).with_context(|| {
        format!(
            "failed to parse original backup manifest {}",
            path.display()
        )
    })
}

fn load_profile_backup_manifest(path: &Path) -> Result<ProfileBackupManifest> {
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read profile backup manifest {}", path.display()))?;
    serde_json::from_str(&payload)
        .with_context(|| format!("failed to parse profile backup manifest {}", path.display()))
}

fn copy_file(source_path: &Path, backup_path: &Path) -> Result<()> {
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create backup directory {}", parent.display()))?;
    }

    fs::copy(source_path, backup_path).with_context(|| {
        format!(
            "failed to copy backup source {} to {}",
            source_path.display(),
            backup_path.display()
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

fn directory_has_entries(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to read directory {}", path.display()))?;
    Ok(entries.next().transpose()?.is_some())
}

fn collect_backup_relative_paths(root: &Path) -> Result<Vec<String>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut stack = vec![root.to_path_buf()];
    let mut output = Vec::new();
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)
            .with_context(|| format!("failed to read directory {}", current.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if path
                .file_name()
                .is_some_and(|value| value == MANIFEST_FILENAME)
            {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            output.push(relative_path);
        }
    }
    output.sort();
    Ok(output)
}

fn result_status(blockers: &[BackupBlocker], ok_status: &str) -> String {
    if blockers.is_empty() {
        ok_status.to_string()
    } else {
        "blocked".to_string()
    }
}

fn warning(code: &str, message: String) -> BackupWarning {
    BackupWarning {
        code: code.to_string(),
        message,
    }
}

fn blocker(code: &str, message: String) -> BackupBlocker {
    BackupBlocker {
        code: code.to_string(),
        message,
    }
}
