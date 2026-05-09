use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::upk::SandboxRebuildValidationResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRecord {
    pub product_id: i64,
    pub name: String,
    pub slot: Option<String>,
    pub slot_id: Option<i64>,
    pub quality: Option<String>,
    pub paintable: bool,
    pub visual_upk: Option<String>,
    pub thumb_upk: Option<String>,
    pub visual_asset: Option<String>,
    pub thumbnail_asset: Option<String>,
    pub product_asset_package: Option<String>,
    pub product_asset_path: Option<String>,
    pub product_thumbnail_package: Option<String>,
    pub product_thumbnail_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotRecord {
    pub slot_id: i64,
    pub name: String,
    pub label: Option<String>,
    pub plural_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintRecord {
    pub paint_id: i64,
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleRecord {
    pub title_id: String,
    pub title_text: String,
    pub category: Option<String>,
    pub color: Option<String>,
    pub glow_color: Option<String>,
    pub sort_priority: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFileRecord {
    pub path: String,
    pub filename: String,
    pub kind: String,
    pub exists_on_disk: bool,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub cooked_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildMethod {
    TargetIdentityRebuild,
    RawRenameCopyForbidden,
}

impl BuildMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TargetIdentityRebuild => "target_identity_rebuild",
            Self::RawRenameCopyForbidden => "raw_rename_copy_forbidden",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapPlanRecord {
    pub plan_id: String,
    pub profile_name: String,
    pub target_product_id: i64,
    pub source_product_id: i64,
    pub build_method: BuildMethod,
    pub target_visual_upk: Option<String>,
    pub target_thumb_upk: Option<String>,
    pub source_visual_upk: Option<String>,
    pub source_thumb_upk: Option<String>,
    pub target_visual_identity: Option<String>,
    pub target_thumb_identity: Option<String>,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedProduct {
    pub id: i64,
    pub name: String,
    pub slot: Option<String>,
    pub slot_id: Option<i64>,
    pub quality: Option<String>,
    pub paintable: bool,
    pub visual_upk: Option<String>,
    pub thumb_upk: Option<String>,
    pub visual_asset: Option<String>,
    pub thumbnail_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapOperation {
    pub kind: String,
    pub enabled: bool,
    pub source_filename: Option<String>,
    pub target_filename: Option<String>,
    pub source_identity: Option<String>,
    pub target_identity: Option<String>,
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub source_sha256: Option<String>,
    pub target_sha256: Option<String>,
    pub backup_path: Option<String>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityCheck {
    pub same_slot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanBuildReport {
    pub build_id: String,
    pub profile_name: String,
    pub status: String,
    pub build_root: String,
    pub visual_output_path: Option<String>,
    pub visual_validation: Option<SandboxRebuildValidationResult>,
    pub thumbnail_output_path: Option<String>,
    pub thumbnail_validation: Option<SandboxRebuildValidationResult>,
    pub warnings: Vec<PlanWarning>,
    pub blockers: Vec<PlanBlocker>,
    pub no_install_performed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapPlan {
    pub plan_id: String,
    pub schema_version: i64,
    pub created_at: DateTime<Utc>,
    pub profile_name: String,
    pub offline_only: bool,
    pub database_path: Option<String>,
    pub configured_cooked_root: Option<String>,
    #[serde(default = "default_plan_status")]
    pub status: String,
    pub target_product: PlannedProduct,
    pub source_product: PlannedProduct,
    pub compatibility: CompatibilityCheck,
    pub operations: Vec<SwapOperation>,
    pub warnings: Vec<PlanWarning>,
    pub build_blockers: Vec<PlanBlocker>,
    #[serde(default)]
    pub last_build: Option<PlanBuildReport>,
    #[serde(default = "default_install_status")]
    pub install_status: String,
    #[serde(default)]
    pub last_install: Option<InstallReport>,
    pub rollback_notes: Vec<String>,
    pub plan_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRecord {
    pub build_id: String,
    pub plan_id: String,
    pub build_root: String,
    pub visual_output_path: Option<String>,
    pub thumb_output_path: Option<String>,
    pub body_matches_source: bool,
    pub target_identity_present: bool,
    pub modified_export_refs_detected: bool,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPreviewFile {
    pub kind: String,
    pub target_filename: String,
    pub target_path: String,
    pub built_path: String,
    pub destination_exists: bool,
    pub built_exists: bool,
    pub would_overwrite: bool,
    pub current_destination_sha256: Option<String>,
    pub planned_original_sha256: Option<String>,
    pub built_output_sha256: Option<String>,
    pub current_matches_planned_original: Option<bool>,
    pub current_matches_built_output: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPreview {
    pub backup_kind: String,
    pub operation_kind: String,
    pub target_relative_path: String,
    pub backup_path: String,
    pub exists: bool,
    pub would_create: bool,
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPreview {
    pub plan_path: String,
    pub profile_name: String,
    pub status: String,
    pub configured_cooked_root: String,
    pub workspace_root: String,
    pub build_root: String,
    pub files: Vec<InstallPreviewFile>,
    pub profile_backups: Vec<BackupPreview>,
    pub permanent_original_backups: Vec<BackupPreview>,
    pub original_backup_manifest_path: String,
    pub restore_command: String,
    pub confirmation_phrase: String,
    pub dry_run_only: bool,
    pub no_files_written: bool,
    pub warnings: Vec<InstallWarning>,
    pub blockers: Vec<InstallBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledFileRecord {
    pub kind: String,
    pub relative_path: String,
    pub target_path: String,
    pub built_path: String,
    pub profile_backup_path: String,
    pub original_backup_path: String,
    pub original_sha256: Option<String>,
    pub built_sha256: Option<String>,
    pub installed_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub plan_id: String,
    pub plan_path: String,
    pub profile_name: String,
    pub status: String,
    pub installed: bool,
    pub installed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub restored_at: Option<DateTime<Utc>>,
    pub cooked_root: String,
    pub profile_backup_manifest_path: String,
    pub original_backup_manifest_path: String,
    pub install_manifest_path: Option<String>,
    pub overwrite_profile_backup: bool,
    pub files: Vec<InstalledFileRecord>,
    pub warnings: Vec<InstallWarning>,
    pub blockers: Vec<InstallBlocker>,
    pub restore_command: String,
    pub confirmation_phrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreFileRecord {
    pub kind: String,
    pub relative_path: String,
    pub backup_kind: String,
    pub backup_source_path: String,
    pub destination_path: String,
    pub destination_exists: bool,
    pub expected_sha256: String,
    pub backup_sha256: Option<String>,
    pub backup_hash_matches_expected: Option<bool>,
    pub actual_restored_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub plan_id: Option<String>,
    pub plan_path: Option<String>,
    pub profile_name: String,
    pub status: String,
    pub dry_run: bool,
    pub from_originals: bool,
    pub restored: bool,
    pub restored_at: Option<DateTime<Utc>>,
    pub cooked_root: String,
    pub install_manifest_path: Option<String>,
    pub profile_backup_manifest_path: String,
    pub original_backup_manifest_path: String,
    pub files: Vec<RestoreFileRecord>,
    pub warnings: Vec<RestoreWarning>,
    pub blockers: Vec<RestoreBlocker>,
    pub restore_command: String,
    pub confirmation_phrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalBackupManifest {
    #[serde(default = "default_backup_schema_version")]
    pub schema_version: i64,
    #[serde(default)]
    pub files: BTreeMap<String, OriginalBackupEntry>,
}

impl Default for OriginalBackupManifest {
    fn default() -> Self {
        Self {
            schema_version: default_backup_schema_version(),
            files: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalBackupEntry {
    pub target_relative_path: String,
    pub target_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileBackupManifest {
    #[serde(default = "default_backup_schema_version")]
    pub schema_version: i64,
    pub profile_name: String,
    pub plan_path: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub overwritten_existing: bool,
    #[serde(default)]
    pub files: BTreeMap<String, ProfileBackupEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileBackupEntry {
    pub operation_kind: String,
    pub target_relative_path: String,
    pub target_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileResult {
    pub backup_kind: String,
    pub operation_kind: String,
    pub target_relative_path: String,
    pub source_path: String,
    pub backup_path: String,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub backup_kind: String,
    pub status: String,
    pub profile_name: Option<String>,
    pub backup_root: String,
    pub manifest_path: String,
    pub files: Vec<BackupFileResult>,
    pub created_count: usize,
    pub existing_count: usize,
    pub verified_count: usize,
    pub warnings: Vec<BackupWarning>,
    pub blockers: Vec<BackupBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVerificationResult {
    pub backup_kind: String,
    pub status: String,
    pub backup_root: String,
    pub manifest_path: String,
    pub tracked_file_count: usize,
    pub missing_file_count: usize,
    pub mismatched_file_count: usize,
    pub untracked_file_count: usize,
    pub files: Vec<BackupFileResult>,
    pub warnings: Vec<BackupWarning>,
    pub blockers: Vec<BackupBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSwapRecord {
    pub install_id: String,
    pub plan_id: String,
    pub profile_name: String,
    pub cooked_root: String,
    pub manifest_path: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub restored_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub dry_run_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalBackupRecord {
    pub backup_id: String,
    pub target_relative_path: String,
    pub backup_path: String,
    pub sha256: String,
    pub backup_kind: String,
    pub profile_name: Option<String>,
    pub cooked_root: String,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsRecord {
    pub key: String,
    pub value_json: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePathValidation {
    pub input_path: String,
    pub normalized_cooked_dir: Option<String>,
    pub input_kind: Option<String>,
    pub is_valid: bool,
    pub input_exists: bool,
    pub cooked_exists: bool,
    pub upk_count: usize,
    pub sample_upks: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatus {
    pub configured_game_path: Option<String>,
    pub configured_cooked_dir: Option<String>,
    pub configured_codered_dumps_dir: Option<String>,
    pub database_ready: bool,
    pub local_files_indexed: bool,
    #[serde(default)]
    pub local_files_count: usize,
    pub product_count: usize,
    pub title_count: usize,
    pub active_swap_count: usize,
    pub original_backup_count: usize,
    pub profile_backup_count: usize,
}

fn default_plan_status() -> String {
    "planned".to_string()
}

fn default_install_status() -> String {
    "not_installed".to_string()
}

fn default_backup_schema_version() -> i64 {
    1
}
