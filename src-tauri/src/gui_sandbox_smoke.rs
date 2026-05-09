use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bakkeswap_core::database::{CodeRedImportSource, SearchRequest};
use bakkeswap_core::services::{
    BuildPlanRequest, InstallExecutionRequest, InstallPreviewRequest, RestoreExecutionRequest,
    RestorePreviewRequest,
};
use bakkeswap_core::upk::compression::{
    compress_body_to_chunk, serialize_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
};
use bakkeswap_core::upk::tables::encrypt_table_region;
use bakkeswap_core::upk::{
    DependsTable, ExportEntry, ExportTable, ImportTable, NameEntry, NameReference,
    RocketLeagueCompressedChunk,
};
use pollster::block_on;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::commands;

const APP_HOME_ENV: &str = "BAKKESWAP_APP_HOME";
const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

#[derive(Debug, Serialize)]
struct SmokeReport {
    sandbox_root: String,
    app_home: String,
    game_root_input: String,
    cooked_dir: String,
    dumps_dir: String,
    import_summary_products: usize,
    refresh_indexed_files: usize,
    target_hits: usize,
    source_hits: usize,
    plan_profile: String,
    plan_same_slot: bool,
    build_status: String,
    install_preview_status: String,
    install_confirmation_phrase: String,
    install_status: String,
    active_swaps_after_install: usize,
    restore_preview_status: String,
    restore_confirmation_phrase: String,
    restore_status: String,
    active_swaps_after_restore: usize,
    inactive_swaps_after_restore: usize,
    backup_status: String,
    backup_verify_status: String,
    original_backup_root: String,
    smoke_commands: Vec<String>,
}

#[derive(Debug)]
struct SandboxLayout {
    sandbox_root: PathBuf,
    app_home: PathBuf,
    game_root_input: PathBuf,
    cooked_dir: PathBuf,
    dumps_dir: PathBuf,
    visual_target_path: PathBuf,
    visual_target_hash: String,
}

struct AppHomeGuard {
    previous: Option<std::ffi::OsString>,
}

impl AppHomeGuard {
    fn set(path: &Path) -> Self {
        let previous = env::var_os(APP_HOME_ENV);
        env::set_var(APP_HOME_ENV, path);
        Self { previous }
    }
}

impl Drop for AppHomeGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            env::set_var(APP_HOME_ENV, previous);
        } else {
            env::remove_var(APP_HOME_ENV);
        }
    }
}

#[test]
fn controlled_gui_sandbox_smoke_flow() {
    block_on(async {
        let sandbox = prepare_gui_smoke_sandbox().expect("prepare gui smoke sandbox");
        let _guard = AppHomeGuard::set(&sandbox.app_home);

        let validation =
            commands::validate_game_path(sandbox.game_root_input.display().to_string())
                .await
                .expect("validate game path");
        assert!(validation.is_valid, "expected fake game path to validate");
        assert_eq!(
            validation.normalized_cooked_dir.as_deref(),
            Some(sandbox.cooked_dir.display().to_string().as_str())
        );

        let saved_validation =
            commands::set_game_path(sandbox.game_root_input.display().to_string())
                .await
                .expect("save game path");
        assert!(saved_validation.is_valid);

        let import_summary = commands::import_codered(CodeRedImportSource {
            folder: sandbox.dumps_dir.display().to_string(),
        })
        .await
        .expect("import fake CodeRed dumps");
        assert_eq!(import_summary.imported_products, 4);

        let refresh = commands::refresh_db().await.expect("refresh db");
        assert_eq!(refresh.status.local_files_count, 5);
        assert_eq!(
            refresh
                .local_index_summary
                .as_ref()
                .map(|value| value.indexed_files),
            Some(5)
        );

        let target_hits = commands::search_items(SearchRequest {
            query: "Target Decal".to_string(),
            limit: 10,
        })
        .await
        .expect("search target");
        let target_hit = target_hits
            .iter()
            .find(|hit| {
                hit.kind == bakkeswap_core::database::SearchKind::Product && hit.id == "1001"
            })
            .expect("target hit");

        let source_hits = commands::search_items(SearchRequest {
            query: "Source Decal".to_string(),
            limit: 10,
        })
        .await
        .expect("search source");
        let source_hit = source_hits
            .iter()
            .find(|hit| {
                hit.kind == bakkeswap_core::database::SearchKind::Product && hit.id == "1002"
            })
            .expect("source hit");

        let plan = commands::create_plan(1001, 1002)
            .await
            .expect("create plan");
        assert!(
            plan.compatibility.same_slot,
            "expected same-slot fixture plan"
        );

        let build_report = commands::build_plan(BuildPlanRequest {
            plan_path: PathBuf::from(&plan.plan_path),
            output_root: None,
            create_dir: true,
        })
        .await
        .expect("build plan");
        assert_eq!(build_report.status, "built");

        let install_preview = commands::install_preview(InstallPreviewRequest {
            plan_path: PathBuf::from(&plan.plan_path),
            build_report: Some(build_report.clone()),
            configured_cooked_root: None,
            workspace_root: None,
            dry_run: true,
        })
        .await
        .expect("install preview");
        assert_eq!(install_preview.status, "preview_ready");
        assert!(install_preview.blockers.is_empty());

        let install_report = commands::install_confirmed(InstallExecutionRequest {
            plan_path: PathBuf::from(&plan.plan_path),
            build_report: Some(build_report.clone()),
            configured_cooked_root: None,
            workspace_root: None,
            confirmation: Some(install_preview.confirmation_phrase.clone()),
            overwrite_profile_backup: false,
        })
        .await
        .expect("install confirmed");
        assert!(install_report.installed);

        let swaps_after_install = commands::list_installed_swaps()
            .await
            .expect("installed swaps after install");
        assert_eq!(swaps_after_install.len(), 1);
        assert!(swaps_after_install[0].active);

        let install_hash = hash_path(&sandbox.visual_target_path).expect("installed target hash");
        let built_visual_path = PathBuf::from(
            build_report
                .visual_output_path
                .clone()
                .expect("visual build output path"),
        );
        let built_hash = hash_path(&built_visual_path).expect("built hash");
        assert_eq!(install_hash, built_hash);

        let restore_preview = commands::restore_preview(RestorePreviewRequest {
            profile_name: plan.profile_name.clone(),
            from_originals: false,
            configured_cooked_root: None,
            workspace_root: None,
        })
        .await
        .expect("restore preview");
        assert_eq!(restore_preview.status, "preview_ready");
        assert!(restore_preview.blockers.is_empty());

        let restore_report = commands::restore_confirmed(RestoreExecutionRequest {
            profile_name: plan.profile_name.clone(),
            from_originals: false,
            confirmation: Some(restore_preview.confirmation_phrase.clone()),
            configured_cooked_root: None,
            workspace_root: None,
        })
        .await
        .expect("restore confirmed");
        assert!(restore_report.restored);

        let restored_hash = hash_path(&sandbox.visual_target_path).expect("restored target hash");
        assert_eq!(restored_hash, sandbox.visual_target_hash);

        let swaps_after_restore = commands::list_installed_swaps()
            .await
            .expect("installed swaps after restore");
        assert_eq!(swaps_after_restore.len(), 1);
        assert!(!swaps_after_restore[0].active);
        assert!(swaps_after_restore[0].restored_at.is_some());

        let backup_status = commands::backup_originals_status()
            .await
            .expect("backup originals status");
        assert_eq!(backup_status.tracked_file_count, 2);
        assert_eq!(backup_status.missing_file_count, 0);

        let backup_verify = commands::backup_originals_verify()
            .await
            .expect("backup originals verify");
        assert_eq!(backup_verify.tracked_file_count, 2);
        assert_eq!(backup_verify.missing_file_count, 0);
        assert_eq!(backup_verify.mismatched_file_count, 0);

        let report = SmokeReport {
            sandbox_root: sandbox.sandbox_root.display().to_string(),
            app_home: sandbox.app_home.display().to_string(),
            game_root_input: sandbox.game_root_input.display().to_string(),
            cooked_dir: sandbox.cooked_dir.display().to_string(),
            dumps_dir: sandbox.dumps_dir.display().to_string(),
            import_summary_products: import_summary.imported_products,
            refresh_indexed_files: refresh.status.local_files_count,
            target_hits: target_hits.len(),
            source_hits: source_hits.len(),
            plan_profile: plan.profile_name.clone(),
            plan_same_slot: plan.compatibility.same_slot,
            build_status: build_report.status,
            install_preview_status: install_preview.status,
            install_confirmation_phrase: install_preview.confirmation_phrase,
            install_status: install_report.status,
            active_swaps_after_install: swaps_after_install
                .iter()
                .filter(|swap| swap.active)
                .count(),
            restore_preview_status: restore_preview.status,
            restore_confirmation_phrase: restore_preview.confirmation_phrase,
            restore_status: restore_report.status,
            active_swaps_after_restore: swaps_after_restore
                .iter()
                .filter(|swap| swap.active)
                .count(),
            inactive_swaps_after_restore: swaps_after_restore
                .iter()
                .filter(|swap| !swap.active)
                .count(),
            backup_status: backup_status.status,
            backup_verify_status: backup_verify.status,
            original_backup_root: backup_verify.backup_root,
            smoke_commands: vec![
                "validate_game_path".to_string(),
                "set_game_path".to_string(),
                "import_codered".to_string(),
                "refresh_db".to_string(),
                format!("search_items target:{}", target_hit.id),
                format!("search_items source:{}", source_hit.id),
                "create_plan".to_string(),
                "build_plan".to_string(),
                "install_preview".to_string(),
                "install_confirmed".to_string(),
                "list_installed_swaps".to_string(),
                "restore_preview".to_string(),
                "restore_confirmed".to_string(),
                "backup_originals_status".to_string(),
                "backup_originals_verify".to_string(),
            ],
        };
        let report_path = sandbox.sandbox_root.join("smoke_report.json");
        fs::write(
            &report_path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&report).expect("serialize report")
            ),
        )
        .expect("write smoke report");
    });
}

fn prepare_gui_smoke_sandbox() -> Result<SandboxLayout> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("resolve workspace root from src-tauri")?;
    let sandbox_root = repo_root.join("target").join("gui_smoke");
    if sandbox_root.exists() {
        fs::remove_dir_all(&sandbox_root).with_context(|| {
            format!(
                "remove previous gui smoke sandbox at {}",
                sandbox_root.display()
            )
        })?;
    }

    let app_home = sandbox_root.join("app_home");
    let game_root_input = sandbox_root.join("RocketLeague");
    let cooked_dir = game_root_input.join("TAGame").join("CookedPCConsole");
    let dumps_dir = sandbox_root.join("codered_dumps");
    fs::create_dir_all(&app_home)?;
    fs::create_dir_all(&cooked_dir)?;
    fs::create_dir_all(&dumps_dir)?;

    copy_fixture_dumps(repo_root, &dumps_dir)?;
    let visual_target_hash = write_synthetic_packages(&cooked_dir)?;

    Ok(SandboxLayout {
        sandbox_root,
        app_home,
        game_root_input,
        cooked_dir: cooked_dir.clone(),
        dumps_dir,
        visual_target_path: cooked_dir.join("Skin_Target_SF.upk"),
        visual_target_hash,
    })
}

fn copy_fixture_dumps(repo_root: &Path, dumps_dir: &Path) -> Result<()> {
    let fixture_root = repo_root
        .join("crates")
        .join("bakkeswap-core")
        .join("tests")
        .join("fixtures")
        .join("codered_planner");
    for filename in [
        "ProductDump.json",
        "SlotDump.json",
        "PaintDump.json",
        "TitleDump.json",
    ] {
        fs::copy(fixture_root.join(filename), dumps_dir.join(filename)).with_context(|| {
            format!(
                "copy fake dump fixture {filename} into {}",
                dumps_dir.display()
            )
        })?;
    }
    Ok(())
}

fn write_synthetic_packages(cooked_dir: &Path) -> Result<String> {
    let packages = [
        (
            "Skin_Target_SF.upk",
            "Skin_Target",
            700,
            b"target visual body".as_slice(),
        ),
        (
            "Skin_Target_T_SF.upk",
            "Skin_Target_T",
            710,
            b"target thumb body".as_slice(),
        ),
        (
            "Skin_Source_SF.upk",
            "Skin_Source",
            720,
            b"source visual body".as_slice(),
        ),
        (
            "Skin_Source_T_SF.upk",
            "Skin_Source_T",
            730,
            b"source thumb body".as_slice(),
        ),
        (
            "Antenna_Source_SF.upk",
            "Antenna_Source",
            740,
            b"mismatched antenna body".as_slice(),
        ),
    ];

    let mut visual_target_hash = None;
    for (filename, identity, serial_offset, body) in packages {
        let bytes = build_test_package(identity, serial_offset, body)?;
        fs::write(cooked_dir.join(filename), &bytes)
            .with_context(|| format!("write synthetic package {filename}"))?;
        if filename == "Skin_Target_SF.upk" {
            visual_target_hash = Some(hash_bytes(&bytes));
        }
    }

    visual_target_hash.context("record visual target hash")
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn hash_path(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hash_bytes(&bytes))
}

fn build_test_package(identity: &str, serial_offset: i64, body: &[u8]) -> Result<Vec<u8>> {
    let mut names = vec![NameEntry {
        index: 0,
        offset: 0,
        name: identity.to_string(),
        flags: 0,
    }];
    reindex_name_entries(&mut names)?;

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
            serial_size: i32::try_from(body.len()).context("body length fits in i32")?,
            serial_offset,
            export_flags: 0,
            net_objects: Vec::new(),
            package_guid: "00000000000000000000000000000000".to_string(),
            package_flags: 0,
        }],
    };
    let depends = DependsTable::default();

    let name_offset = synthetic_summary_size() as i32;
    let name_bytes = serialize_names(&names)?;
    let import_bytes = serialize_imports(&imports);
    let export_bytes = serialize_exports(&exports.entries, 22)?;
    let depends_bytes = serialize_depends(&depends);
    let chunk_payload = compress_body_to_chunk(body, DEFAULT_RL_BLOCK_SIZE)?;

    let import_offset = name_offset + i32::try_from(name_bytes.len()).context("name bytes fit")?;
    let export_offset =
        import_offset + i32::try_from(import_bytes.len()).context("import bytes fit")?;
    let depends_offset =
        export_offset + i32::try_from(export_bytes.len()).context("export bytes fit")?;
    let chunk_meta_offset =
        name_bytes.len() + import_bytes.len() + export_bytes.len() + depends_bytes.len();
    let placeholder_chunks = vec![RocketLeagueCompressedChunk {
        uncompressed_offset: i64::from(depends_offset),
        uncompressed_size: i32::try_from(body.len()).context("body len fits i32")?,
        compressed_offset: 0,
        compressed_size: i32::try_from(chunk_payload.len())
            .context("chunk payload len fits i32")?,
    }];

    let mut plain_prefix = Vec::new();
    plain_prefix.extend_from_slice(&name_bytes);
    plain_prefix.extend_from_slice(&import_bytes);
    plain_prefix.extend_from_slice(&export_bytes);
    plain_prefix.extend_from_slice(&depends_bytes);
    plain_prefix.extend_from_slice(&serialize_rl_compressed_chunks(&placeholder_chunks)?);
    let logical_length = logical_length_for_tables(plain_prefix.len());
    let encrypted_size = align16(logical_length);
    let total_header_size = name_offset
        + i32::try_from(logical_length).context("logical length fits i32")?
        + DEFAULT_TEST_GARBAGE_SIZE;

    let chunks = vec![RocketLeagueCompressedChunk {
        uncompressed_offset: i64::from(depends_offset),
        uncompressed_size: i32::try_from(body.len()).context("body len fits i32")?,
        compressed_offset: i64::from(total_header_size),
        compressed_size: i32::try_from(chunk_payload.len()).context("chunk payload fits i32")?,
    }];
    let mut plain_logical = Vec::new();
    plain_logical.extend_from_slice(&name_bytes);
    plain_logical.extend_from_slice(&import_bytes);
    plain_logical.extend_from_slice(&export_bytes);
    plain_logical.extend_from_slice(&depends_bytes);
    plain_logical.extend_from_slice(&serialize_rl_compressed_chunks(&chunks)?);
    plain_logical.resize(logical_length, 0);

    let mut plain_encrypted = plain_logical;
    plain_encrypted.resize(encrypted_size, 0);
    let encrypted = encrypt_table_region(&plain_encrypted)?;
    let physical_garbage_len = usize::try_from(DEFAULT_TEST_GARBAGE_SIZE)
        .context("default garbage size fits usize")?
        - (encrypted_size - logical_length);

    let mut raw = build_summary_header(
        name_offset,
        total_header_size,
        import_offset,
        export_offset,
        depends_offset,
        i32::try_from(chunk_meta_offset).context("chunk meta offset fits i32")?,
        i32::try_from(body.len()).context("body len fits i32")?,
        1,
        0,
        1,
    );
    raw.extend_from_slice(&encrypted);
    raw.extend_from_slice(&vec![0u8; physical_garbage_len]);
    raw.extend_from_slice(&chunk_payload);
    Ok(raw)
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
            let offset = i32::try_from(item.serial_offset).context("serial offset fits i32")?;
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
    bytes.extend_from_slice(
        &(i32::try_from(value.len()).context("fstring len fits i32")? + 1).to_le_bytes(),
    );
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
