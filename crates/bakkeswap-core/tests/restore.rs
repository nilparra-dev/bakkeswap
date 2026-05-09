use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bakkeswap_core::database::DatabaseService;
use bakkeswap_core::domain::models::{
    CompatibilityCheck, InstallReport, PlanBlocker, PlannedProduct, ProfileBackupEntry,
    ProfileBackupManifest, RestoreReport, SwapOperation, SwapPlan,
};
use bakkeswap_core::services::{
    BuildPlanRequest, BuildService, InstallExecutionRequest, InstallerService,
    RestoreExecutionRequest, RestorePreviewRequest, RestoreService,
};
use bakkeswap_core::upk::compression::{
    compress_body_to_chunk, serialize_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
};
use bakkeswap_core::upk::tables::encrypt_table_region;
use bakkeswap_core::upk::{
    DependsTable, ExportEntry, ExportTable, ImportTable, NameEntry, NameReference,
    RocketLeagueCompressedChunk,
};
use chrono::Utc;
use rusqlite::params;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const COOKED_DIR_KEY: &str = "cooked_dir";
const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

struct RestoreFixture {
    profile_name: String,
    database: DatabaseService,
    plan_path: PathBuf,
    cooked_root: PathBuf,
    workspace_root: PathBuf,
    install_manifest_path: PathBuf,
    profile_manifest_path: PathBuf,
    visual_destination_path: PathBuf,
    thumbnail_destination_path: Option<PathBuf>,
    visual_profile_backup_path: PathBuf,
    visual_original_hash: String,
    thumbnail_original_hash: Option<String>,
}

#[test]
fn successful_restore_dry_run() {
    let (_temp, fixture) = prepare_restore_fixture("restore_dry_run_profile", false, false);
    let before_hash = hash_path(&fixture.visual_destination_path);

    let report = preview_restore(&fixture, false);

    assert_eq!(report.status, "preview_ready");
    assert!(report.dry_run);
    assert!(!report.restored);
    assert_eq!(report.files.len(), 1);
    assert_eq!(
        report.files[0].expected_sha256,
        fixture.visual_original_hash
    );
    assert_eq!(
        report.files[0].backup_sha256.as_deref(),
        Some(fixture.visual_original_hash.as_str())
    );
    assert_eq!(hash_path(&fixture.visual_destination_path), before_hash);
}

#[test]
fn successful_restore_from_profile_backup() {
    let (_temp, fixture) = prepare_restore_fixture("restore_profile_success", true, false);

    let report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );

    assert_eq!(report.status, "restored");
    assert!(report.restored);
    assert_eq!(report.files.len(), 2);
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        fixture.visual_original_hash
    );
    assert_eq!(
        hash_path(fixture.thumbnail_destination_path.as_ref().unwrap()),
        fixture.thumbnail_original_hash.clone().unwrap()
    );
}

#[test]
fn wrong_confirmation_refuses_restore() {
    let (_temp, fixture) = prepare_restore_fixture("restore_wrong_confirmation", false, false);
    let before_hash = hash_path(&fixture.visual_destination_path);

    let report = execute_restore(&fixture, Some("RESTORE nope".to_string()), false);

    assert_eq!(report.status, "blocked");
    assert!(!report.restored);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "confirmation_mismatch"));
    assert_eq!(hash_path(&fixture.visual_destination_path), before_hash);
}

#[test]
fn missing_confirmation_refuses_restore() {
    let (_temp, fixture) = prepare_restore_fixture("restore_missing_confirmation", false, false);
    let before_hash = hash_path(&fixture.visual_destination_path);

    let report = execute_restore(&fixture, None, false);

    assert_eq!(report.status, "blocked");
    assert!(!report.restored);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "confirmation_required"));
    assert_eq!(hash_path(&fixture.visual_destination_path), before_hash);
}

#[test]
fn missing_profile_backup_blocks_restore() {
    let (_temp, fixture) = prepare_restore_fixture("restore_missing_profile_backup", false, false);
    fs::remove_file(&fixture.visual_profile_backup_path).unwrap();

    let report = preview_restore(&fixture, false);

    assert_eq!(report.status, "blocked");
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_backup_missing_file"));
}

#[test]
fn tampered_backup_hash_blocks_restore() {
    let (_temp, fixture) = prepare_restore_fixture("restore_tampered_backup", false, false);
    fs::write(
        &fixture.visual_profile_backup_path,
        b"tampered profile backup",
    )
    .unwrap();

    let report = preview_restore(&fixture, false);

    assert_eq!(report.status, "blocked");
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_backup_hash_mismatch"));
}

#[test]
fn path_escape_blocks_restore() {
    let (_temp, fixture) = prepare_restore_fixture("restore_escape_guard", false, false);
    let mut manifest: ProfileBackupManifest =
        serde_json::from_str(&fs::read_to_string(&fixture.profile_manifest_path).unwrap()).unwrap();
    let mut entry = manifest.files.remove("TargetIdentity_SF.upk").unwrap();
    entry.target_relative_path = "../escape.upk".to_string();
    manifest
        .files
        .insert("../escape.upk".to_string(), ProfileBackupEntry { ..entry });
    fs::write(
        &fixture.profile_manifest_path,
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let report = preview_restore(&fixture, false);

    assert_eq!(report.status, "blocked");
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_relative_path_invalid"));
    assert!(!fixture
        .cooked_root
        .parent()
        .unwrap()
        .join("escape.upk")
        .exists());
}

#[test]
fn restored_destination_hash_verified() {
    let (_temp, fixture) = prepare_restore_fixture("restore_hash_verified", false, false);

    let report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );

    assert_eq!(report.status, "restored");
    assert_eq!(
        report.files[0].actual_restored_sha256.as_deref(),
        Some(fixture.visual_original_hash.as_str())
    );
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        fixture.visual_original_hash
    );
}

#[test]
fn install_manifest_restored_at_updated() {
    let (_temp, fixture) = prepare_restore_fixture("restore_manifest_updated", false, false);

    let _report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );

    let install_manifest: InstallReport =
        serde_json::from_str(&fs::read_to_string(&fixture.install_manifest_path).unwrap()).unwrap();
    assert!(install_manifest.restored_at.is_some());
}

#[test]
fn saved_plan_install_status_updated() {
    let (_temp, fixture) = prepare_restore_fixture("restore_plan_updated", false, false);

    let _report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );

    let plan = read_plan(&fixture.plan_path);
    assert_eq!(plan.install_status, "restored");
    assert!(plan.last_install.as_ref().unwrap().restored_at.is_some());
}

#[test]
fn sqlite_installed_swap_restored_at_and_active_updated() {
    let (_temp, fixture) = prepare_restore_fixture("restore_sqlite_updated", false, true);

    let report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );

    assert_eq!(report.status, "restored");
    let connection = fixture.database.connect().unwrap();
    let (restored_at, active): (Option<String>, i64) = connection
        .query_row(
            "SELECT restored_at, active FROM installed_swaps WHERE plan_id = ?1",
            params![report.plan_id.as_deref().unwrap()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(restored_at.is_some());
    assert_eq!(active, 0);
}

#[test]
fn originals_fallback_refused_without_flag_and_correct_confirmation() {
    let (_temp, fixture) = prepare_restore_fixture("restore_originals_refused", false, false);
    fs::remove_file(&fixture.profile_manifest_path).unwrap();

    let normal_report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        false,
    );
    assert_eq!(normal_report.status, "blocked");
    assert!(normal_report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_backup_manifest_missing"));

    let originals_report = execute_restore(
        &fixture,
        Some(expected_restore_confirmation(&fixture)),
        true,
    );
    assert_eq!(originals_report.status, "blocked");
    assert!(originals_report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "confirmation_mismatch"));
}

#[test]
fn emergency_fallback_succeeds_when_profile_backup_missing_and_originals_valid() {
    let (_temp, fixture) = prepare_restore_fixture("restore_originals_success", false, false);
    fs::remove_file(&fixture.profile_manifest_path).unwrap();
    fs::remove_file(&fixture.install_manifest_path).unwrap();

    let report = execute_restore(
        &fixture,
        Some(expected_originals_confirmation(&fixture)),
        true,
    );

    assert_eq!(report.status, "restored_with_warnings");
    assert!(report.restored);
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == "emergency_original_restore_requested"));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.code == "install_manifest_missing"));
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        fixture.visual_original_hash
    );
}

fn preview_restore(fixture: &RestoreFixture, from_originals: bool) -> RestoreReport {
    RestoreService::new(fixture.database.clone())
        .preview_restore(&RestorePreviewRequest {
            profile_name: fixture.profile_name.clone(),
            from_originals,
            workspace_root: Some(fixture.workspace_root.clone()),
            ..RestorePreviewRequest::default()
        })
        .unwrap()
}

fn execute_restore(
    fixture: &RestoreFixture,
    confirmation: Option<String>,
    from_originals: bool,
) -> RestoreReport {
    RestoreService::new(fixture.database.clone())
        .restore(&RestoreExecutionRequest {
            profile_name: fixture.profile_name.clone(),
            from_originals,
            confirmation,
            workspace_root: Some(fixture.workspace_root.clone()),
            ..RestoreExecutionRequest::default()
        })
        .unwrap()
}

fn expected_install_confirmation(profile_name: &str) -> String {
    format!("INSTALL {}", profile_name)
}

fn expected_restore_confirmation(fixture: &RestoreFixture) -> String {
    format!("RESTORE {}", fixture.profile_name)
}

fn expected_originals_confirmation(fixture: &RestoreFixture) -> String {
    format!("RESTORE ORIGINALS {}", fixture.profile_name)
}

fn prepare_restore_fixture(
    profile_name: &str,
    include_thumbnail: bool,
    persist_plan_record: bool,
) -> (TempDir, RestoreFixture) {
    let (temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    let cooked_root = database
        .app_home()
        .join("RocketLeague")
        .join("TAGame")
        .join("CookedPCConsole");
    let workspace_root = database.app_home().join("workspace");
    fs::create_dir_all(&package_dir).unwrap();
    fs::create_dir_all(&cooked_root).unwrap();
    database
        .set_string_setting(COOKED_DIR_KEY, &cooked_root.display().to_string())
        .unwrap();

    let visual_source_bytes = build_test_package("SourceIdentity", 777, b"source body");
    let visual_target_bytes = build_test_package("TargetIdentity", 888, b"target body");
    let visual_source_path = package_dir.join("SourceIdentity_SF.upk");
    let visual_target_path = package_dir.join("TargetIdentity_SF.upk");
    let visual_destination_path = cooked_root.join("TargetIdentity_SF.upk");
    fs::write(&visual_source_path, &visual_source_bytes).unwrap();
    fs::write(&visual_target_path, &visual_target_bytes).unwrap();
    fs::write(&visual_destination_path, &visual_target_bytes).unwrap();
    let visual_original_hash = hash_bytes(&visual_target_bytes);

    let mut operations = vec![operation(
        "visual",
        Some(&visual_source_path),
        Some(&visual_target_path),
        Some(&visual_source_bytes),
        Some(&visual_target_bytes),
        true,
    )];

    let mut thumbnail_destination_path = None;
    let mut thumbnail_original_hash = None;
    if include_thumbnail {
        let thumbnail_source_bytes =
            build_test_package("SourceIdentity_T", 977, b"source thumb body");
        let thumbnail_target_bytes =
            build_test_package("TargetIdentity_T", 1088, b"target thumb body");
        let thumbnail_source_path = package_dir.join("SourceIdentity_T_SF.upk");
        let thumbnail_target_path = package_dir.join("TargetIdentity_T_SF.upk");
        let destination_path = cooked_root.join("TargetIdentity_T_SF.upk");
        fs::write(&thumbnail_source_path, &thumbnail_source_bytes).unwrap();
        fs::write(&thumbnail_target_path, &thumbnail_target_bytes).unwrap();
        fs::write(&destination_path, &thumbnail_target_bytes).unwrap();
        operations.push(operation(
            "thumbnail",
            Some(&thumbnail_source_path),
            Some(&thumbnail_target_path),
            Some(&thumbnail_source_bytes),
            Some(&thumbnail_target_bytes),
            true,
        ));
        thumbnail_destination_path = Some(destination_path);
        thumbnail_original_hash = Some(hash_bytes(&thumbnail_target_bytes));
    } else {
        operations.push(operation("thumbnail", None, None, None, None, false));
    }

    let plan = build_plan(
        &database,
        profile_name,
        &cooked_root,
        operations,
        Vec::new(),
    );
    let plan_path = write_plan_file(&database, plan);
    if persist_plan_record {
        persist_plan_in_database(&database, &plan_path);
    }

    let _build_report = BuildService::new(database.clone())
        .build_plan(&BuildPlanRequest {
            plan_path: plan_path.clone(),
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    let install_report = InstallerService::new(database.clone())
        .install(&InstallExecutionRequest {
            plan_path: plan_path.clone(),
            configured_cooked_root: Some(cooked_root.clone()),
            workspace_root: Some(workspace_root.clone()),
            confirmation: Some(expected_install_confirmation(profile_name)),
            overwrite_profile_backup: false,
            ..InstallExecutionRequest::default()
        })
        .unwrap();
    assert!(install_report.installed);

    let install_manifest_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join("install_manifest.json");
    let profile_manifest_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join("manifest.json");
    let visual_profile_backup_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join("TargetIdentity_SF.upk");

    (
        temp,
        RestoreFixture {
            profile_name: profile_name.to_string(),
            database,
            plan_path,
            cooked_root,
            workspace_root,
            install_manifest_path,
            profile_manifest_path,
            visual_destination_path,
            thumbnail_destination_path,
            visual_profile_backup_path,
            visual_original_hash,
            thumbnail_original_hash,
        },
    )
}

fn temp_database() -> (TempDir, DatabaseService) {
    let temp = TempDir::new().expect("temporary test directory");
    let database = DatabaseService::from_app_home(temp.path().join("app_home"));
    database.connect().expect("database initialization");
    (temp, database)
}

fn build_plan(
    database: &DatabaseService,
    profile_name: &str,
    cooked_root: &Path,
    operations: Vec<SwapOperation>,
    build_blockers: Vec<PlanBlocker>,
) -> SwapPlan {
    let plan_path = database
        .app_home()
        .join("workspace")
        .join("plans")
        .join(profile_name)
        .join("swap_plan.json");
    let visual_operation = operations
        .iter()
        .find(|operation| operation.kind == "visual");
    let thumbnail_operation = operations
        .iter()
        .find(|operation| operation.kind == "thumbnail");

    SwapPlan {
        plan_id: format!("plan_{profile_name}"),
        schema_version: 1,
        created_at: Utc::now(),
        profile_name: profile_name.to_string(),
        offline_only: true,
        database_path: Some(database.database_path().display().to_string()),
        configured_cooked_root: Some(cooked_root.display().to_string()),
        status: if build_blockers.is_empty() {
            "planned".to_string()
        } else {
            "blocked".to_string()
        },
        install_status: "not_installed".to_string(),
        target_product: planned_product(
            1001,
            "Target Product",
            visual_operation,
            thumbnail_operation,
            false,
        ),
        source_product: planned_product(
            1002,
            "Source Product",
            visual_operation,
            thumbnail_operation,
            true,
        ),
        compatibility: CompatibilityCheck { same_slot: true },
        operations,
        warnings: Vec::new(),
        build_blockers,
        last_build: None,
        last_install: None,
        rollback_notes: Vec::new(),
        plan_path: plan_path.display().to_string(),
    }
}

fn planned_product(
    id: i64,
    name: &str,
    visual_operation: Option<&SwapOperation>,
    thumbnail_operation: Option<&SwapOperation>,
    source: bool,
) -> PlannedProduct {
    PlannedProduct {
        id,
        name: name.to_string(),
        slot: Some("Decal".to_string()),
        slot_id: Some(1),
        quality: None,
        paintable: false,
        visual_upk: visual_operation.and_then(|operation| {
            if source {
                operation.source_filename.clone()
            } else {
                operation.target_filename.clone()
            }
        }),
        thumb_upk: thumbnail_operation.and_then(|operation| {
            if source {
                operation.source_filename.clone()
            } else {
                operation.target_filename.clone()
            }
        }),
        visual_asset: None,
        thumbnail_asset: None,
    }
}

fn operation(
    kind: &str,
    source_path: Option<&Path>,
    target_path: Option<&Path>,
    source_bytes: Option<&[u8]>,
    target_bytes: Option<&[u8]>,
    enabled: bool,
) -> SwapOperation {
    SwapOperation {
        kind: kind.to_string(),
        enabled,
        source_filename: source_path.and_then(filename_string),
        target_filename: target_path.and_then(filename_string),
        source_identity: source_path.and_then(identity_string),
        target_identity: target_path.and_then(identity_string),
        source_path: source_path.map(|path| path.display().to_string()),
        target_path: target_path.map(|path| path.display().to_string()),
        source_sha256: source_bytes.map(hash_bytes),
        target_sha256: target_bytes.map(hash_bytes),
        backup_path: None,
        output_path: None,
    }
}

fn filename_string(path: &Path) -> Option<String> {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
}

fn identity_string(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    if stem.to_ascii_lowercase().ends_with("_sf") {
        return Some(stem[..stem.len().saturating_sub(3)].to_string());
    }
    Some(stem)
}

fn write_plan_file(database: &DatabaseService, plan: SwapPlan) -> PathBuf {
    let plan_path = PathBuf::from(&plan.plan_path);
    fs::create_dir_all(plan_path.parent().unwrap()).unwrap();
    fs::write(
        &plan_path,
        format!("{}\n", serde_json::to_string_pretty(&plan).unwrap()),
    )
    .unwrap();
    let _ = database;
    plan_path
}

fn read_plan(plan_path: &Path) -> SwapPlan {
    serde_json::from_str(&fs::read_to_string(plan_path).unwrap()).unwrap()
}

fn persist_plan_in_database(database: &DatabaseService, plan_path: &Path) {
    let plan = read_plan(plan_path);
    let connection = database.connect().unwrap();
    let timestamp = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT OR IGNORE INTO products (product_id, name, updated_at) VALUES (?1, ?2, ?3)",
            params![
                plan.target_product.id,
                plan.target_product.name,
                timestamp.clone()
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT OR IGNORE INTO products (product_id, name, updated_at) VALUES (?1, ?2, ?3)",
            params![
                plan.source_product.id,
                plan.source_product.name,
                timestamp.clone()
            ],
        )
        .unwrap();

    let visual_operation = plan
        .operations
        .iter()
        .find(|operation| operation.kind == "visual");
    let thumbnail_operation = plan
        .operations
        .iter()
        .find(|operation| operation.kind == "thumbnail");

    connection
        .execute(
            "INSERT INTO swap_plans (
                plan_id, profile_name, target_product_id, source_product_id,
                target_visual_upk, target_thumb_upk, source_visual_upk, source_thumb_upk,
                target_visual_identity, target_thumb_identity, build_method, plan_path,
                cooked_root, notes_json, created_at, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                plan.plan_id,
                plan.profile_name,
                plan.target_product.id,
                plan.source_product.id,
                plan.target_product.visual_upk,
                plan.target_product.thumb_upk,
                plan.source_product.visual_upk,
                plan.source_product.thumb_upk,
                visual_operation.and_then(|operation| operation.target_identity.clone()),
                thumbnail_operation.and_then(|operation| operation.target_identity.clone()),
                "sandbox_rebuild",
                plan.plan_path,
                plan.configured_cooked_root,
                serde_json::to_string(&serde_json::json!({})).unwrap(),
                plan.created_at.to_rfc3339(),
                plan.status,
            ],
        )
        .unwrap();
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn hash_path(path: &Path) -> String {
    hash_bytes(&fs::read(path).unwrap())
}

fn build_test_package(identity: &str, serial_offset: i64, body: &[u8]) -> Vec<u8> {
    let mut names = vec![NameEntry {
        index: 0,
        offset: 0,
        name: identity.to_string(),
        flags: 0,
    }];
    reindex_name_entries(&mut names).unwrap();

    let imports = ImportTable::default();
    let exports = ExportTable {
        entries: vec![ExportEntry {
            index: 0,
            class_index: 0,
            super_index: 0,
            outer_index: 0,
            object_name: NameReference {
                name_index: 0,
                instance_number: 0,
                name: Some(identity.to_string()),
            },
            archetype_index: 0,
            object_flags: 0,
            serial_size: i32::try_from(body.len()).unwrap(),
            serial_offset,
            export_flags: 0,
            net_objects: Vec::new(),
            package_guid: "00000000000000000000000000000000".to_string(),
            package_flags: 0,
        }],
    };
    let depends = DependsTable::default();

    let name_offset = synthetic_summary_size() as i32;
    let name_bytes = serialize_names(&names).unwrap();
    let import_bytes = serialize_imports(&imports);
    let export_bytes = serialize_exports(&exports.entries, 22).unwrap();
    let depends_bytes = serialize_depends(&depends);
    let chunk_payload = compress_body_to_chunk(body, DEFAULT_RL_BLOCK_SIZE).unwrap();

    let import_offset = name_offset + i32::try_from(name_bytes.len()).unwrap();
    let export_offset = import_offset + i32::try_from(import_bytes.len()).unwrap();
    let depends_offset = export_offset + i32::try_from(export_bytes.len()).unwrap();
    let chunk_meta_offset =
        name_bytes.len() + import_bytes.len() + export_bytes.len() + depends_bytes.len();
    let placeholder_chunks = vec![RocketLeagueCompressedChunk {
        uncompressed_offset: i64::from(depends_offset),
        uncompressed_size: i32::try_from(body.len()).unwrap(),
        compressed_offset: 0,
        compressed_size: i32::try_from(chunk_payload.len()).unwrap(),
    }];

    let mut plain_prefix = Vec::new();
    plain_prefix.extend_from_slice(&name_bytes);
    plain_prefix.extend_from_slice(&import_bytes);
    plain_prefix.extend_from_slice(&export_bytes);
    plain_prefix.extend_from_slice(&depends_bytes);
    plain_prefix.extend_from_slice(&serialize_rl_compressed_chunks(&placeholder_chunks).unwrap());
    let logical_length = logical_length_for_tables(plain_prefix.len());
    let encrypted_size = align16(logical_length);
    let total_header_size =
        name_offset + i32::try_from(logical_length).unwrap() + DEFAULT_TEST_GARBAGE_SIZE;

    let chunks = vec![RocketLeagueCompressedChunk {
        uncompressed_offset: i64::from(depends_offset),
        uncompressed_size: i32::try_from(body.len()).unwrap(),
        compressed_offset: i64::from(total_header_size),
        compressed_size: i32::try_from(chunk_payload.len()).unwrap(),
    }];
    let mut plain_logical = Vec::new();
    plain_logical.extend_from_slice(&name_bytes);
    plain_logical.extend_from_slice(&import_bytes);
    plain_logical.extend_from_slice(&export_bytes);
    plain_logical.extend_from_slice(&depends_bytes);
    plain_logical.extend_from_slice(&serialize_rl_compressed_chunks(&chunks).unwrap());
    plain_logical.resize(logical_length, 0);

    let mut plain_encrypted = plain_logical;
    plain_encrypted.resize(encrypted_size, 0);
    let encrypted = encrypt_table_region(&plain_encrypted).unwrap();
    let physical_garbage_len =
        usize::try_from(DEFAULT_TEST_GARBAGE_SIZE).unwrap() - (encrypted_size - logical_length);

    let mut raw = build_summary_header(
        name_offset,
        total_header_size,
        import_offset,
        export_offset,
        depends_offset,
        i32::try_from(chunk_meta_offset).unwrap(),
        i32::try_from(body.len()).unwrap(),
        1,
        0,
        1,
    );
    raw.extend_from_slice(&encrypted);
    raw.extend_from_slice(&vec![0u8; physical_garbage_len]);
    raw.extend_from_slice(&chunk_payload);
    raw
}

fn serialize_names(names: &[NameEntry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in names {
        bytes.extend_from_slice(&pack_fstring(&entry.name)?);
        bytes.extend_from_slice(&entry.flags.to_le_bytes());
    }
    Ok(bytes)
}

fn serialize_imports(imports: &ImportTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    for item in &imports.entries {
        bytes.extend_from_slice(&pack_name_reference(&item.class_package));
        bytes.extend_from_slice(&pack_name_reference(&item.class_name));
        bytes.extend_from_slice(&item.outer_index.to_le_bytes());
        bytes.extend_from_slice(&pack_name_reference(&item.object_name));
    }
    bytes
}

fn serialize_exports(exports: &[ExportEntry], licensee_version: u16) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for item in exports {
        bytes.extend_from_slice(&item.class_index.to_le_bytes());
        bytes.extend_from_slice(&item.super_index.to_le_bytes());
        bytes.extend_from_slice(&item.outer_index.to_le_bytes());
        bytes.extend_from_slice(&pack_name_reference(&item.object_name));
        bytes.extend_from_slice(&item.archetype_index.to_le_bytes());
        bytes.extend_from_slice(&item.object_flags.to_le_bytes());
        bytes.extend_from_slice(&item.serial_size.to_le_bytes());
        if licensee_version >= 22 {
            bytes.extend_from_slice(&item.serial_offset.to_le_bytes());
        } else {
            let offset = i32::try_from(item.serial_offset)
                .map_err(|_| anyhow!("serial offset does not fit in i32"))?;
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.extend_from_slice(&item.export_flags.to_le_bytes());
        bytes.extend_from_slice(&(item.net_objects.len() as i32).to_le_bytes());
        for net_object in &item.net_objects {
            bytes.extend_from_slice(&net_object.to_le_bytes());
        }
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&item.package_flags.to_le_bytes());
    }
    Ok(bytes)
}

fn serialize_depends(depends: &DependsTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in &depends.entries {
        bytes.extend_from_slice(&entry.value.to_le_bytes());
    }
    bytes
}

fn pack_fstring(value: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(i32::try_from(value.len()).unwrap() + 1).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    Ok(bytes)
}

fn pack_name_reference(reference: &NameReference) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[0..4].copy_from_slice(&reference.name_index.to_le_bytes());
    bytes[4..8].copy_from_slice(&reference.instance_number.to_le_bytes());
    bytes
}

fn reindex_name_entries(names: &mut [NameEntry]) -> Result<()> {
    let mut offset = 0usize;
    for (index, entry) in names.iter_mut().enumerate() {
        entry.index = index;
        entry.offset = offset;
        offset += pack_fstring(&entry.name)?.len() + 8;
    }
    Ok(())
}

fn logical_length_for_tables(plain_len: usize) -> usize {
    if plain_len % 16 == 15 {
        plain_len + 1
    } else {
        plain_len
    }
}

fn align16(value: usize) -> usize {
    (value + 15) & !15
}

fn synthetic_summary_size() -> usize {
    build_summary_header(0, 0, 0, 0, 0, 0, 0, 0, 0, 0).len()
}

#[allow(clippy::too_many_arguments)]
fn build_summary_header(
    name_offset: i32,
    total_header_size: i32,
    import_offset: i32,
    export_offset: i32,
    depends_offset: i32,
    compressed_chunks_offset: i32,
    last_block_size: i32,
    name_count: i32,
    import_count: i32,
    export_count: i32,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x9E2A83C1u32.to_le_bytes());
    bytes.extend_from_slice(&845u16.to_le_bytes());
    bytes.extend_from_slice(&22u16.to_le_bytes());
    bytes.extend_from_slice(&total_header_size.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&name_count.to_le_bytes());
    bytes.extend_from_slice(&name_offset.to_le_bytes());
    bytes.extend_from_slice(&export_count.to_le_bytes());
    bytes.extend_from_slice(&export_offset.to_le_bytes());
    bytes.extend_from_slice(&import_count.to_le_bytes());
    bytes.extend_from_slice(&import_offset.to_le_bytes());
    bytes.extend_from_slice(&depends_offset.to_le_bytes());
    bytes.extend_from_slice(&depends_offset.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&depends_offset.to_le_bytes());
    bytes.extend_from_slice(&[0u8; 16]);
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&DEFAULT_TEST_GARBAGE_SIZE.to_le_bytes());
    bytes.extend_from_slice(&compressed_chunks_offset.to_le_bytes());
    bytes.extend_from_slice(&last_block_size.to_le_bytes());
    bytes
}
