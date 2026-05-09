use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bakkeswap_core::database::DatabaseService;
use bakkeswap_core::domain::models::{
    CompatibilityCheck, PlanBlocker, PlannedProduct, SwapOperation, SwapPlan,
};
use bakkeswap_core::services::{BuildPlanRequest, BuildService};
use bakkeswap_core::upk::compression::{
    compress_body_to_chunk, serialize_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
};
use bakkeswap_core::upk::tables::encrypt_table_region;
use bakkeswap_core::upk::{
    DependsTable, ExportEntry, ExportTable, ImportTable, NameEntry, NameReference,
    RocketLeagueCompressedChunk,
};
use chrono::Utc;
use tempfile::TempDir;

const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

fn temp_database() -> (TempDir, DatabaseService) {
    let temp = TempDir::new().expect("temporary test directory");
    let database = DatabaseService::from_app_home(temp.path().join("app_home"));
    database.connect().expect("database initialization");
    (temp, database)
}

#[test]
fn builds_visual_only_plan_and_updates_plan_json() {
    let (_temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    fs::create_dir_all(&package_dir).unwrap();

    let source_path = package_dir.join("SourceIdentity_SF.upk");
    let target_path = package_dir.join("TargetIdentity_SF.upk");
    fs::write(
        &source_path,
        build_test_package("SourceIdentity", 777, b"source body"),
    )
    .unwrap();
    fs::write(
        &target_path,
        build_test_package("TargetIdentity", 888, b"target body"),
    )
    .unwrap();

    let profile_name = "visual_only_profile";
    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            profile_name,
            vec![
                operation("visual", Some(&source_path), Some(&target_path), true),
                operation("thumbnail", None, None, false),
            ],
            Vec::new(),
        ),
    );

    let report = BuildService::new(database.clone())
        .build_plan(&BuildPlanRequest {
            plan_path: plan_path.clone(),
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    let expected_output = database
        .app_home()
        .join("workspace")
        .join("builds")
        .join(profile_name)
        .join("TargetIdentity_SF.upk");
    assert_eq!(report.status, "built");
    assert_eq!(
        report.visual_output_path.as_deref(),
        Some(expected_output.display().to_string().as_str())
    );
    assert!(expected_output.exists());
    assert!(report.visual_validation.as_ref().unwrap().passed);
    assert!(report.thumbnail_output_path.is_none());
    assert!(report.no_install_performed);

    let updated_plan = read_plan(&plan_path);
    assert_eq!(updated_plan.status, "built");
    assert_eq!(
        updated_plan
            .last_build
            .as_ref()
            .and_then(|build| build.visual_output_path.as_deref()),
        Some(expected_output.display().to_string().as_str())
    );
}

#[test]
fn builds_visual_and_thumbnail_plan_with_explicit_output_root() {
    let (_temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    let output_root = database.app_home().join("local_output");
    fs::create_dir_all(&package_dir).unwrap();

    let source_visual = package_dir.join("SourceIdentity_SF.upk");
    let target_visual = package_dir.join("TargetIdentity_SF.upk");
    let source_thumb = package_dir.join("SourceIdentity_T_SF.upk");
    let target_thumb = package_dir.join("TargetIdentity_T_SF.upk");
    fs::write(
        &source_visual,
        build_test_package("SourceIdentity", 777, b"source body"),
    )
    .unwrap();
    fs::write(
        &target_visual,
        build_test_package("TargetIdentity", 888, b"target body"),
    )
    .unwrap();
    fs::write(
        &source_thumb,
        build_test_package("SourceIdentity_T", 977, b"source thumb body"),
    )
    .unwrap();
    fs::write(
        &target_thumb,
        build_test_package("TargetIdentity_T", 1088, b"target thumb body"),
    )
    .unwrap();

    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            "visual_and_thumbnail_profile",
            vec![
                operation("visual", Some(&source_visual), Some(&target_visual), true),
                operation("thumbnail", Some(&source_thumb), Some(&target_thumb), true),
            ],
            Vec::new(),
        ),
    );

    let report = BuildService::new(database)
        .build_plan(&BuildPlanRequest {
            plan_path,
            output_root: Some(output_root.clone()),
            create_dir: true,
        })
        .unwrap();

    assert_eq!(report.status, "built");
    assert_eq!(
        report.visual_output_path.as_deref(),
        Some(
            output_root
                .join("TargetIdentity_SF.upk")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        report.thumbnail_output_path.as_deref(),
        Some(
            output_root
                .join("TargetIdentity_T_SF.upk")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert!(report.visual_validation.as_ref().unwrap().passed);
    assert!(report.thumbnail_validation.as_ref().unwrap().passed);
}

#[test]
fn blocked_plan_returns_blocked_report() {
    let (_temp, database) = temp_database();
    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            "blocked_profile",
            Vec::new(),
            vec![PlanBlocker {
                code: "slot_mismatch".to_string(),
                message: "Slot mismatch: target=Decal source=Antenna".to_string(),
            }],
        ),
    );

    let report = BuildService::new(database)
        .build_plan(&BuildPlanRequest {
            plan_path: plan_path.clone(),
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    assert_eq!(report.status, "blocked");
    assert!(report.visual_output_path.is_none());
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.code == "slot_mismatch"));

    let updated_plan = read_plan(&plan_path);
    assert_eq!(updated_plan.status, "blocked");
    assert!(updated_plan.last_build.is_some());
}

#[test]
fn rejects_output_root_inside_cookedpcconsole() {
    let (_temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    fs::create_dir_all(&package_dir).unwrap();

    let source_path = package_dir.join("SourceIdentity_SF.upk");
    let target_path = package_dir.join("TargetIdentity_SF.upk");
    fs::write(
        &source_path,
        build_test_package("SourceIdentity", 777, b"source body"),
    )
    .unwrap();
    fs::write(
        &target_path,
        build_test_package("TargetIdentity", 888, b"target body"),
    )
    .unwrap();

    let output_root = database
        .app_home()
        .join("RocketLeague")
        .join("TAGame")
        .join("CookedPCConsole")
        .join("sandbox");
    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            "unsafe_output_profile",
            vec![
                operation("visual", Some(&source_path), Some(&target_path), true),
                operation("thumbnail", None, None, false),
            ],
            Vec::new(),
        ),
    );

    let report = BuildService::new(database)
        .build_plan(&BuildPlanRequest {
            plan_path,
            output_root: Some(output_root),
            create_dir: true,
        })
        .unwrap();

    assert_eq!(report.status, "failed");
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker.message.contains("CookedPCConsole")));
}

#[test]
fn reports_missing_source_package_helpfully() {
    let (_temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    fs::create_dir_all(&package_dir).unwrap();

    let source_path = package_dir.join("MissingSource_SF.upk");
    let target_path = package_dir.join("TargetIdentity_SF.upk");
    fs::write(
        &target_path,
        build_test_package("TargetIdentity", 888, b"target body"),
    )
    .unwrap();

    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            "missing_source_profile",
            vec![
                operation("visual", Some(&source_path), Some(&target_path), true),
                operation("thumbnail", None, None, false),
            ],
            Vec::new(),
        ),
    );

    let report = BuildService::new(database)
        .build_plan(&BuildPlanRequest {
            plan_path,
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    assert_eq!(report.status, "failed");
    assert!(report.blockers.iter().any(|blocker| blocker
        .message
        .contains("source package does not exist on disk")));
}

#[test]
fn reports_missing_target_package_helpfully() {
    let (_temp, database) = temp_database();
    let package_dir = database.app_home().join("local_samples");
    fs::create_dir_all(&package_dir).unwrap();

    let source_path = package_dir.join("SourceIdentity_SF.upk");
    let target_path = package_dir.join("MissingTarget_SF.upk");
    fs::write(
        &source_path,
        build_test_package("SourceIdentity", 777, b"source body"),
    )
    .unwrap();

    let plan_path = write_plan_file(
        &database,
        build_plan(
            &database,
            "missing_target_profile",
            vec![
                operation("visual", Some(&source_path), Some(&target_path), true),
                operation("thumbnail", None, None, false),
            ],
            Vec::new(),
        ),
    );

    let report = BuildService::new(database)
        .build_plan(&BuildPlanRequest {
            plan_path,
            output_root: None,
            create_dir: true,
        })
        .unwrap();

    assert_eq!(report.status, "failed");
    assert!(report.blockers.iter().any(|blocker| blocker
        .message
        .contains("target package does not exist on disk")));
}

fn build_plan(
    database: &DatabaseService,
    profile_name: &str,
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
        configured_cooked_root: None,
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
        source_sha256: None,
        target_sha256: None,
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
