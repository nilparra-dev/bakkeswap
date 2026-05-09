use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bakkeswap_core::database::DatabaseService;
use bakkeswap_core::domain::models::{
    CompatibilityCheck, OriginalBackupManifest, PlanBlocker, PlannedProduct, ProfileBackupManifest,
    SwapOperation, SwapPlan,
};
use bakkeswap_core::services::{
    BuildPlanRequest, BuildService, InstallPreviewRequest, InstallerService,
    PermanentOriginalBackupManager, ProfileBackupManager,
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
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

struct BackupFixture {
    database: DatabaseService,
    plan_path: PathBuf,
    cooked_root: PathBuf,
    workspace_root: PathBuf,
    visual_destination_path: PathBuf,
    thumbnail_destination_path: Option<PathBuf>,
    visual_output_path: PathBuf,
    visual_profile_backup_path: PathBuf,
    thumbnail_profile_backup_path: Option<PathBuf>,
    visual_original_backup_path: PathBuf,
}

#[test]
fn permanent_original_backup_creates_files_and_manifest() {
    let (_temp, fixture) = prepare_backup_fixture("original_create_profile", false, false);
    let preview = preview_install(&fixture);
    let before_destination_hash = hash_path(&fixture.visual_destination_path);

    let result = PermanentOriginalBackupManager::new(fixture.database.clone())
        .prepare_from_preview(&preview)
        .unwrap();

    assert_eq!(result.status, "prepared");
    assert!(result.blockers.is_empty());
    assert_eq!(result.created_count, 1);
    assert_eq!(result.existing_count, 0);
    assert!(fixture.visual_original_backup_path.exists());
    assert_eq!(
        hash_path(&fixture.visual_original_backup_path),
        before_destination_hash
    );
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        before_destination_hash
    );

    let manifest: OriginalBackupManifest =
        serde_json::from_str(&fs::read_to_string(preview.original_backup_manifest_path).unwrap())
            .unwrap();
    let entry = manifest.files.get("TargetIdentity_SF.upk").unwrap();
    assert_eq!(entry.sha256, before_destination_hash);
}

#[test]
fn permanent_original_backup_second_run_does_not_overwrite() {
    let (_temp, fixture) = prepare_backup_fixture("original_second_run_profile", false, false);
    let preview = preview_install(&fixture);
    let manager = PermanentOriginalBackupManager::new(fixture.database.clone());

    manager.prepare_from_preview(&preview).unwrap();
    let backup_hash_before = hash_path(&fixture.visual_original_backup_path);

    let result = manager.prepare_from_preview(&preview).unwrap();

    assert_eq!(result.status, "prepared");
    assert!(result.blockers.is_empty());
    assert_eq!(result.created_count, 0);
    assert_eq!(result.existing_count, 1);
    assert_eq!(result.verified_count, 1);
    assert_eq!(result.files[0].status, "already_exists");
    assert_eq!(
        hash_path(&fixture.visual_original_backup_path),
        backup_hash_before
    );
}

#[test]
fn permanent_original_backup_verify_reports_existing_backup() {
    let (_temp, fixture) = prepare_backup_fixture("original_verify_profile", false, false);
    let preview = preview_install(&fixture);
    let manager = PermanentOriginalBackupManager::new(fixture.database.clone());

    manager.prepare_from_preview(&preview).unwrap();
    let verification = manager.verify().unwrap();

    assert_eq!(verification.status, "ready");
    assert!(verification.blockers.is_empty());
    assert_eq!(verification.tracked_file_count, 1);
    assert!(verification.files.iter().any(|file| {
        file.target_relative_path == "TargetIdentity_SF.upk" && file.status == "verified"
    }));
}

#[test]
fn permanent_original_backup_hash_mismatch_blocks() {
    let (_temp, fixture) = prepare_backup_fixture("original_mismatch_profile", false, false);
    let preview = preview_install(&fixture);
    let manager = PermanentOriginalBackupManager::new(fixture.database.clone());

    manager.prepare_from_preview(&preview).unwrap();
    fs::write(
        &fixture.visual_original_backup_path,
        b"tampered original backup",
    )
    .unwrap();

    let result = manager.prepare_from_preview(&preview).unwrap();

    assert_eq!(result.status, "blocked");
    assert!(result
        .blockers
        .iter()
        .any(|blocker| blocker.code == "original_backup_hash_mismatch"));
}

#[test]
fn profile_backup_creates_files_and_manifest() {
    let (_temp, fixture) = prepare_backup_fixture("profile_create_profile", true, false);
    let preview = preview_install(&fixture);
    let before_visual_hash = hash_path(&fixture.visual_destination_path);
    let before_thumbnail_hash = hash_path(fixture.thumbnail_destination_path.as_ref().unwrap());

    let result = ProfileBackupManager::new(fixture.database.clone())
        .prepare_from_preview(&preview, false)
        .unwrap();

    assert_eq!(result.status, "prepared");
    assert!(result.blockers.is_empty());
    assert_eq!(result.created_count, 2);
    assert!(fixture.visual_profile_backup_path.exists());
    assert!(fixture
        .thumbnail_profile_backup_path
        .as_ref()
        .unwrap()
        .exists());
    assert_eq!(
        hash_path(&fixture.visual_profile_backup_path),
        before_visual_hash
    );
    assert_eq!(
        hash_path(fixture.thumbnail_profile_backup_path.as_ref().unwrap()),
        before_thumbnail_hash
    );
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        before_visual_hash
    );
    assert_eq!(
        hash_path(fixture.thumbnail_destination_path.as_ref().unwrap()),
        before_thumbnail_hash
    );

    let manifest_path = fixture
        .workspace_root
        .join("backups")
        .join("profile_create_profile")
        .join("manifest.json");
    let manifest: ProfileBackupManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.files.len(), 2);
    assert!(manifest.files.contains_key("TargetIdentity_SF.upk"));
    assert!(manifest.files.contains_key("TargetIdentity_T_SF.upk"));
}

#[test]
fn profile_backup_existing_folder_refuses_by_default() {
    let (_temp, fixture) = prepare_backup_fixture("profile_refuse_profile", false, false);
    let preview = preview_install(&fixture);
    let manager = ProfileBackupManager::new(fixture.database.clone());

    manager.prepare_from_preview(&preview, false).unwrap();
    let result = manager.prepare_from_preview(&preview, false).unwrap();

    assert_eq!(result.status, "blocked");
    assert!(result
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_backup_already_exists"));
}

#[test]
fn profile_backup_hash_verification_failure_blocks() {
    let (_temp, fixture) = prepare_backup_fixture("profile_mismatch_profile", false, false);
    let preview = preview_install(&fixture);
    let manager = ProfileBackupManager::new(fixture.database.clone());

    manager.prepare_from_preview(&preview, false).unwrap();
    fs::write(
        &fixture.visual_profile_backup_path,
        b"tampered profile backup",
    )
    .unwrap();

    let result = manager.prepare_from_preview(&preview, true).unwrap();

    assert_eq!(result.status, "blocked");
    assert!(result
        .blockers
        .iter()
        .any(|blocker| blocker.code == "profile_backup_hash_mismatch"));
}

#[test]
fn backup_managers_do_not_install_rebuilt_files_or_modify_destinations() {
    let (_temp, fixture) = prepare_backup_fixture("backup_safety_profile", false, false);
    let preview = preview_install(&fixture);
    let destination_hash_before = hash_path(&fixture.visual_destination_path);
    let build_hash_before = hash_path(&fixture.visual_output_path);
    assert_ne!(destination_hash_before, build_hash_before);

    let originals_result = PermanentOriginalBackupManager::new(fixture.database.clone())
        .prepare_from_preview(&preview)
        .unwrap();
    let profile_result = ProfileBackupManager::new(fixture.database.clone())
        .prepare_from_preview(&preview, false)
        .unwrap();

    assert!(originals_result.blockers.is_empty());
    assert!(profile_result.blockers.is_empty());
    assert_eq!(
        hash_path(&fixture.visual_destination_path),
        destination_hash_before
    );
    assert_eq!(hash_path(&fixture.visual_output_path), build_hash_before);
}

fn preview_install(fixture: &BackupFixture) -> bakkeswap_core::domain::models::InstallPreview {
    InstallerService::new(fixture.database.clone())
        .preview_install(&InstallPreviewRequest {
            plan_path: fixture.plan_path.clone(),
            configured_cooked_root: Some(fixture.cooked_root.clone()),
            workspace_root: Some(fixture.workspace_root.clone()),
            dry_run: true,
            ..InstallPreviewRequest::default()
        })
        .unwrap()
}

fn prepare_backup_fixture(
    profile_name: &str,
    include_thumbnail: bool,
    drift_visual_destination: bool,
) -> (TempDir, BackupFixture) {
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

    let visual_source_bytes = build_test_package("SourceIdentity", 777, b"source body");
    let visual_target_bytes = build_test_package("TargetIdentity", 888, b"target body");
    let drifted_visual_target_bytes = build_test_package("TargetIdentity", 888, b"drifted body");
    let visual_source_path = package_dir.join("SourceIdentity_SF.upk");
    let visual_target_path = package_dir.join("TargetIdentity_SF.upk");
    let visual_destination_path = cooked_root.join("TargetIdentity_SF.upk");
    fs::write(&visual_source_path, &visual_source_bytes).unwrap();
    fs::write(&visual_target_path, &visual_target_bytes).unwrap();
    fs::write(
        &visual_destination_path,
        if drift_visual_destination {
            &drifted_visual_target_bytes
        } else {
            &visual_target_bytes
        },
    )
    .unwrap();

    let mut operations = vec![operation(
        "visual",
        Some(&visual_source_path),
        Some(&visual_target_path),
        Some(&visual_source_bytes),
        Some(&visual_target_bytes),
        true,
    )];

    let mut thumbnail_destination_path = None;
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
    let report = BuildService::new(database.clone())
        .build_plan(&BuildPlanRequest {
            plan_path: plan_path.clone(),
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    let visual_output_path = PathBuf::from(report.visual_output_path.unwrap());
    let visual_profile_backup_path = workspace_root
        .join("backups")
        .join(profile_name)
        .join("TargetIdentity_SF.upk");
    let thumbnail_profile_backup_path = include_thumbnail.then(|| {
        workspace_root
            .join("backups")
            .join(profile_name)
            .join("TargetIdentity_T_SF.upk")
    });
    let visual_original_backup_path = workspace_root
        .join("original_files_backup")
        .join("TargetIdentity_SF.upk");

    (
        temp,
        BackupFixture {
            database,
            plan_path,
            cooked_root,
            workspace_root,
            visual_destination_path,
            thumbnail_destination_path,
            visual_output_path,
            visual_profile_backup_path,
            thumbnail_profile_backup_path,
            visual_original_backup_path,
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
