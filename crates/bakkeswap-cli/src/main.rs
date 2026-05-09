use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use bakkeswap_core::database::{
    CodeRedImportSource, DatabaseImporter, DatabaseService, LocalFileIndexer, SearchEngine,
    SearchKind, SearchRequest,
};
use bakkeswap_core::domain::models::{
    BackupResult, BackupVerificationResult, InstallPreview, InstallReport, PlanBuildReport,
    RestoreReport, SwapPlan,
};
use bakkeswap_core::services::{
    BuildPlanRequest, BuildService, InstallExecutionRequest, InstallPreviewRequest,
    InstallerService, PathService, PermanentOriginalBackupManager, PlannerService,
    ProfileBackupManager, RestoreExecutionRequest, RestorePreviewRequest, RestoreService,
    StatusService,
};
use bakkeswap_core::upk::{
    rebuild_target_identity, KnownAnswerHarness, KnownAnswerReport, KnownAnswerRequest,
    SandboxRebuildOptions, SandboxRebuildReport, TableCountSnapshot, UpkInspectReport,
    UpkInspector,
};
use clap::{Args, Parser, Subcommand};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(name = "bakkeswap", version, about = "BakkesSwap Rust CLI skeleton")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Selftest,
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Search(SearchArgs),
    Upk {
        #[command(subcommand)]
        command: UpkCommand,
    },
    Plan(PlanArgs),
    Build(BuildArgs),
    Install(InstallArgs),
    Restore(RestoreArgs),
    Status,
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    SetGamePath { path: PathBuf },
    Show,
    Validate,
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    ImportCodered { folder: PathBuf },
    Refresh,
}

#[derive(Debug, Subcommand)]
enum UpkCommand {
    Inspect(UpkInspectArgs),
    KnownAnswer(UpkKnownAnswerArgs),
    RebuildSandbox(UpkRebuildSandboxArgs),
}

#[derive(Debug, Args)]
struct SearchArgs {
    query: String,
    #[arg(long, default_value_t = 25)]
    limit: usize,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct UpkInspectArgs {
    path: PathBuf,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct UpkKnownAnswerArgs {
    #[arg(long)]
    source: PathBuf,
    #[arg(long)]
    target: PathBuf,
    #[arg(long)]
    expected: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    create_dir: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct UpkRebuildSandboxArgs {
    #[arg(long)]
    source: PathBuf,
    #[arg(long)]
    target: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = false)]
    create_dir: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct PlanArgs {
    #[arg(long)]
    target: i64,
    #[arg(long)]
    source: i64,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[arg(long)]
    plan: PathBuf,
    #[arg(long)]
    output_root: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    create_dir: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct InstallArgs {
    #[arg(long)]
    plan: PathBuf,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    #[arg(long)]
    confirm: Option<String>,
    #[arg(long, default_value_t = false)]
    overwrite_profile_backup: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct RestoreArgs {
    #[arg(long)]
    profile: String,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    #[arg(long)]
    confirm: Option<String>,
    #[arg(long, default_value_t = false)]
    from_originals: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    Originals {
        #[command(subcommand)]
        command: BackupOriginalsCommand,
    },
    Profile {
        #[command(subcommand)]
        command: BackupProfileCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BackupOriginalsCommand {
    Status,
    Verify,
    Prepare(BackupPrepareArgs),
}

#[derive(Debug, Subcommand)]
enum BackupProfileCommand {
    Prepare(ProfileBackupPrepareArgs),
}

#[derive(Debug, Args)]
struct BackupPrepareArgs {
    #[arg(long)]
    plan: PathBuf,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct ProfileBackupPrepareArgs {
    #[arg(long)]
    plan: PathBuf,
    #[arg(long, default_value_t = false)]
    overwrite_profile_backup: bool,
    #[arg(long, default_value_t = false)]
    json: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Selftest => command_selftest(),
        Command::Config { command } => match command {
            ConfigCommand::SetGamePath { path } => command_config_set_game_path(path),
            ConfigCommand::Show => command_config_show(),
            ConfigCommand::Validate => command_config_validate(),
        },
        Command::Db { command } => match command {
            DbCommand::ImportCodered { folder } => command_db_import_codered(folder),
            DbCommand::Refresh => command_db_refresh(),
        },
        Command::Search(args) => command_search(args),
        Command::Upk { command } => match command {
            UpkCommand::Inspect(args) => command_upk_inspect(args),
            UpkCommand::KnownAnswer(args) => command_upk_known_answer(args),
            UpkCommand::RebuildSandbox(args) => command_upk_rebuild_sandbox(args),
        },
        Command::Plan(args) => command_plan(args),
        Command::Build(args) => command_build(args),
        Command::Install(args) => command_install(args),
        Command::Restore(args) => command_restore(args),
        Command::Status => command_status(),
        Command::Backup { command } => match command {
            BackupCommand::Originals { command } => match command {
                BackupOriginalsCommand::Status => command_backup_originals_status(),
                BackupOriginalsCommand::Verify => command_backup_originals_verify(),
                BackupOriginalsCommand::Prepare(args) => command_backup_originals_prepare(args),
            },
            BackupCommand::Profile { command } => match command {
                BackupProfileCommand::Prepare(args) => command_backup_profile_prepare(args),
            },
        },
    }
}

fn command_selftest() -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    database.connect()?;
    let status = StatusService::new(database.clone()).current_status()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "command": "selftest",
            "status": "ok",
            "app_home": database.app_home().display().to_string(),
            "database_path": database.database_path().display().to_string(),
            "runtime_status": status,
        }))?
    );
    Ok(())
}

fn command_config_set_game_path(path: PathBuf) -> Result<()> {
    let service = PathService::new(DatabaseService::for_current_user()?);
    let validation = service.set_game_path(&path.display().to_string())?;
    println!("{}", serde_json::to_string_pretty(&validation)?);
    Ok(())
}

fn command_config_show() -> Result<()> {
    let snapshot = PathService::new(DatabaseService::for_current_user()?).show_config()?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}

fn command_config_validate() -> Result<()> {
    let validation =
        PathService::new(DatabaseService::for_current_user()?).validate_configured_game_path()?;
    println!("{}", serde_json::to_string_pretty(&validation)?);
    if !validation.is_valid {
        std::process::exit(2);
    }
    Ok(())
}

fn command_db_import_codered(folder: PathBuf) -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let importer = DatabaseImporter::new(database);
    let summary = importer.import_codered(&CodeRedImportSource {
        folder: folder.display().to_string(),
    })?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn command_db_refresh() -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let importer = DatabaseImporter::new(database.clone());
    let import_summary = importer.refresh()?;
    let path_service = PathService::new(database.clone());
    let local_index_summary = match path_service.configured_cooked_dir()? {
        Some(cooked_dir) => Some(LocalFileIndexer::new(database).index_cooked_dir(&cooked_dir)?),
        None => None,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "command": "db refresh",
            "import_summary": import_summary,
            "local_file_index_summary": local_index_summary,
        }))?
    );
    Ok(())
}

fn command_search(args: SearchArgs) -> Result<()> {
    let engine = SearchEngine::new(DatabaseService::for_current_user()?);
    let hits = engine.search_products(&SearchRequest {
        query: args.query,
        limit: args.limit,
    })?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else {
        print_search_table(&hits);
    }
    Ok(())
}

fn command_upk_inspect(args: UpkInspectArgs) -> Result<()> {
    let report = UpkInspector.inspect_path(&args.path)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_upk_inspect_summary(&report);
    }
    Ok(())
}

fn command_upk_known_answer(args: UpkKnownAnswerArgs) -> Result<()> {
    let harness = KnownAnswerHarness::default();
    let configured_cooked_dir = configured_cooked_dir_string()?;
    let report = harness.analyze(&KnownAnswerRequest {
        source_path: args.source,
        target_path: args.target,
        expected_path: args.expected,
        generated_output_path: args.output,
        sandbox_output_root: None,
        sandbox_rebuild_options: SandboxRebuildOptions {
            create_dir: args.create_dir,
            configured_cooked_dir,
            ..SandboxRebuildOptions::default()
        },
    })?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_known_answer_summary(&report);
    }
    Ok(())
}

fn command_upk_rebuild_sandbox(args: UpkRebuildSandboxArgs) -> Result<()> {
    let report = rebuild_target_identity(
        &args.source,
        &args.target,
        &args.output,
        &SandboxRebuildOptions {
            create_dir: args.create_dir,
            configured_cooked_dir: configured_cooked_dir_string()?,
            ..SandboxRebuildOptions::default()
        },
    )?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_sandbox_rebuild_summary(&report);
    }
    Ok(())
}

fn command_status() -> Result<()> {
    let status = StatusService::new(DatabaseService::for_current_user()?).current_status()?;
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

fn command_plan(args: PlanArgs) -> Result<()> {
    let planner = PlannerService::new(DatabaseService::for_current_user()?);
    let plan = planner.create_plan(args.target, args.source)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_plan_summary(&plan);
    }
    Ok(())
}

fn command_build(args: BuildArgs) -> Result<()> {
    let report =
        BuildService::new(DatabaseService::for_current_user()?).build_plan(&BuildPlanRequest {
            plan_path: args.plan,
            output_root: args.output_root,
            create_dir: args.create_dir,
        })?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_build_summary(&report);
    }

    if report.status != "built" {
        std::process::exit(3);
    }

    Ok(())
}

fn command_install(args: InstallArgs) -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let installer = InstallerService::new(database);
    let plan_path = args.plan;

    if args.dry_run {
        let preview = installer.preview_install(&InstallPreviewRequest {
            plan_path,
            dry_run: true,
            ..InstallPreviewRequest::default()
        })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&preview)?);
        } else {
            print_install_preview_summary(&preview);
        }

        if preview.status != "preview_ready" {
            std::process::exit(4);
        }

        return Ok(());
    }

    let mut confirmation = args.confirm;
    if confirmation.is_none() && !args.json {
        let preview = installer.preview_install(&InstallPreviewRequest {
            plan_path: plan_path.clone(),
            dry_run: true,
            ..InstallPreviewRequest::default()
        })?;

        print_install_preview_summary(&preview);
        if preview.status != "preview_ready" {
            std::process::exit(4);
        }

        confirmation = prompt_install_confirmation(&preview.confirmation_phrase)?;
        if confirmation.is_none() {
            println!("Install cancelled.");
            std::process::exit(4);
        }
    }

    let report = installer.install(&InstallExecutionRequest {
        plan_path,
        confirmation,
        overwrite_profile_backup: args.overwrite_profile_backup,
        ..InstallExecutionRequest::default()
    })?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_install_report_summary(&report);
    }

    if report.status != "installed" && report.status != "installed_with_warnings" {
        std::process::exit(4);
    }

    Ok(())
}

fn command_restore(args: RestoreArgs) -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let restorer = RestoreService::new(database);

    if args.dry_run {
        let report = restorer.preview_restore(&RestorePreviewRequest {
            profile_name: args.profile,
            from_originals: args.from_originals,
            ..RestorePreviewRequest::default()
        })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print_restore_report_summary(&report);
        }

        if report.status != "preview_ready" {
            std::process::exit(7);
        }

        return Ok(());
    }

    let report = restorer.restore(&RestoreExecutionRequest {
        profile_name: args.profile,
        from_originals: args.from_originals,
        confirmation: args.confirm,
        ..RestoreExecutionRequest::default()
    })?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_restore_report_summary(&report);
    }

    if report.status != "restored" && report.status != "restored_with_warnings" {
        std::process::exit(7);
    }

    Ok(())
}

fn command_backup_originals_status() -> Result<()> {
    let result =
        PermanentOriginalBackupManager::new(DatabaseService::for_current_user()?).status()?;
    print_backup_verification_summary(&result);
    Ok(())
}

fn command_backup_originals_verify() -> Result<()> {
    let result =
        PermanentOriginalBackupManager::new(DatabaseService::for_current_user()?).verify()?;
    print_backup_verification_summary(&result);
    if result.status != "ready" {
        std::process::exit(6);
    }
    Ok(())
}

fn command_backup_originals_prepare(args: BackupPrepareArgs) -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let preview = load_install_preview(database.clone(), args.plan)?;
    let result = PermanentOriginalBackupManager::new(database).prepare_from_preview(&preview)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_backup_result_summary(&result);
    }

    if result.status != "prepared" {
        std::process::exit(5);
    }

    Ok(())
}

fn command_backup_profile_prepare(args: ProfileBackupPrepareArgs) -> Result<()> {
    let database = DatabaseService::for_current_user()?;
    let preview = load_install_preview(database.clone(), args.plan)?;
    let result = ProfileBackupManager::new(database)
        .prepare_from_preview(&preview, args.overwrite_profile_backup)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_backup_result_summary(&result);
    }

    if result.status != "prepared" {
        std::process::exit(5);
    }

    Ok(())
}

fn load_install_preview(database: DatabaseService, plan: PathBuf) -> Result<InstallPreview> {
    InstallerService::new(database).preview_install(&InstallPreviewRequest {
        plan_path: plan,
        dry_run: true,
        ..InstallPreviewRequest::default()
    })
}

fn print_search_table(hits: &[bakkeswap_core::database::SearchHit]) {
    if hits.is_empty() {
        println!("No results.");
        return;
    }

    println!(
        "{:<8} {:<18} {:<38} {:<18} {:<24} {:<24} {:<12}",
        "Kind", "ID", "Name", "Slot", "Visual Package", "Thumb Package", "Swappable"
    );
    println!("{}", "-".repeat(150));

    for hit in hits {
        let kind = match hit.kind {
            SearchKind::Product => "product",
            SearchKind::Title => "title",
        };
        println!(
            "{:<8} {:<18} {:<38} {:<18} {:<24} {:<24} {:<12}",
            kind,
            hit.id,
            truncate(&hit.name, 38),
            hit.slot.clone().unwrap_or_else(|| "-".to_string()),
            hit.product_asset_package
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            hit.product_thumbnail_package
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            if hit.swappable { "yes" } else { "no" }
        );
        if let Some(note) = &hit.note {
            println!("  note: {note}");
        }
    }
}

fn print_plan_summary(plan: &SwapPlan) {
    println!("Plan written: {}", plan.plan_path);
    println!("Profile: {}", plan.profile_name);
    println!(
        "Target:  {}  {}  {}",
        plan.target_product.id,
        plan.target_product.name,
        plan.target_product
            .visual_upk
            .clone()
            .unwrap_or_else(|| "[no visual]".to_string())
    );
    println!(
        "Source:  {}  {}  {}",
        plan.source_product.id,
        plan.source_product.name,
        plan.source_product
            .visual_upk
            .clone()
            .unwrap_or_else(|| "[no visual]".to_string())
    );
    println!(
        "Cooked root: {}",
        plan.configured_cooked_root
            .clone()
            .unwrap_or_else(|| "[not configured]".to_string())
    );
    println!("Operations:");
    for operation in &plan.operations {
        println!(
            "  - {}: {} <- {}{}",
            operation.kind,
            operation
                .target_filename
                .clone()
                .unwrap_or_else(|| "[missing target]".to_string()),
            operation
                .source_filename
                .clone()
                .unwrap_or_else(|| "[missing source]".to_string()),
            if operation.enabled { "" } else { " [disabled]" }
        );
    }
    if plan.build_blockers.is_empty() {
        println!("Build blockers: none");
    } else {
        println!("Build blockers:");
        for blocker in &plan.build_blockers {
            println!("  - {}", blocker.message);
        }
    }
    if plan.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &plan.warnings {
            println!("  - {}", warning.message);
        }
    }
}

fn print_upk_inspect_summary(report: &UpkInspectReport) {
    println!("File: {}", report.path);
    println!("Size: {} bytes", report.file_size);
    println!("SHA256: {}", report.sha256);
    println!(
        "Version: file={} licensee={}",
        report.file_version, report.licensee_version
    );
    println!("Magic: {}", report.magic);
    println!(
        "Rocket League UE3: {}",
        if report.is_probable_rocket_league {
            "yes"
        } else {
            "no"
        }
    );
    println!("Header size: {}", report.total_header_size);
    println!("Package flags: {}", report.package_flags);
    println!("Compression flags: {}", report.compression_flags);
    println!(
        "Counts: names={} imports={} exports={} depends={}",
        report.name_count,
        report.import_count,
        report.export_count,
        report
            .depends_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "[not parsed]".to_string())
    );
    println!(
        "Compressed chunks: {}",
        report
            .compressed_chunk_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "[not parsed]".to_string())
    );
    println!(
        "Decompressed body: size={} sha256={}",
        report
            .decompressed_body_size
            .map(|value| value.to_string())
            .unwrap_or_else(|| "[not available]".to_string()),
        report
            .decompressed_body_sha256
            .clone()
            .unwrap_or_else(|| "[not available]".to_string())
    );
    println!(
        "Status: summary={} rl={} decrypted={} names={} imports={} exports={} depends={} chunks={} body={}",
        yes_no(report.status.summary_parsed),
        yes_no(report.status.detected_rocket_league_format),
        yes_no(report.status.tables_decrypted),
        yes_no(report.status.name_table_parsed),
        yes_no(report.status.import_table_parsed),
        yes_no(report.status.export_table_parsed),
        yes_no(report.status.depends_table_parsed),
        yes_no(report.status.compressed_chunks_parsed),
        yes_no(report.status.body_decompressed),
    );
    if report.string_evidence.is_empty() {
        println!("String evidence: none");
    } else {
        println!("String evidence:");
        for value in &report.string_evidence {
            println!("  - {}", value);
        }
    }
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning);
        }
    }
}

fn print_build_summary(report: &PlanBuildReport) {
    println!("Plan build:");
    println!("Profile: {}", report.profile_name);
    println!("Status: {}", report.status);
    println!("Build root: {}", report.build_root);
    println!(
        "No install performed: {}",
        yes_no(report.no_install_performed)
    );
    println!(
        "Visual output: {}",
        report
            .visual_output_path
            .as_deref()
            .unwrap_or("[not built]")
    );
    if let Some(validation) = &report.visual_validation {
        println!(
            "Visual validation: passed={} filename={} body={} target_name={} target_refs={}",
            yes_no(validation.passed),
            yes_no(validation.filename_matches_target),
            yes_no(validation.body_equals_source),
            yes_no(validation.target_name_present),
            validation.target_export_name_count,
        );
    } else {
        println!("Visual validation: [not available]");
    }
    println!(
        "Thumbnail output: {}",
        report
            .thumbnail_output_path
            .as_deref()
            .unwrap_or("[not built]")
    );
    if let Some(validation) = &report.thumbnail_validation {
        println!(
            "Thumbnail validation: passed={} filename={} body={} target_name={} target_refs={}",
            yes_no(validation.passed),
            yes_no(validation.filename_matches_target),
            yes_no(validation.body_equals_source),
            yes_no(validation.target_name_present),
            validation.target_export_name_count,
        );
    } else {
        println!("Thumbnail validation: [not available]");
    }

    if report.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &report.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning.message);
        }
    }
}

fn print_install_preview_summary(preview: &InstallPreview) {
    let backup_dir = PathBuf::from(&preview.workspace_root)
        .join("backups")
        .join(&preview.profile_name);

    println!("Install profile: {}", preview.profile_name);
    println!(
        "Configured CookedPCConsole: {}",
        preview.configured_cooked_root
    );
    println!("Backup folder: {}", backup_dir.display());
    if preview.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &preview.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if preview.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &preview.warnings {
            println!("  - {}", warning.message);
        }
    }

    println!("CookedPCConsole files that will be overwritten:");
    if preview.files.is_empty() {
        println!("  [none]");
    } else {
        for file in &preview.files {
            println!("  {}: {}", file.kind, file.target_path);
        }
    }

    println!("Rebuilt source files that will be installed:");
    if preview.files.is_empty() {
        println!("  [none]");
    } else {
        for file in &preview.files {
            println!("  {}: {}", file.kind, file.built_path);
        }
    }

    println!("Backup targets:");
    if preview.profile_backups.is_empty() {
        println!("  [none]");
    } else {
        for backup in &preview.profile_backups {
            println!(
                "  {}: {} ({})",
                backup.operation_kind, backup.backup_path, backup.status
            );
        }
    }

    println!("Permanent original backups:");
    if preview.permanent_original_backups.is_empty() {
        println!("  [none]");
    } else {
        for backup in &preview.permanent_original_backups {
            println!(
                "  {}: {} ({})",
                backup.operation_kind, backup.backup_path, backup.status
            );
            for warning in &backup.warnings {
                println!("    warning: {}", warning);
            }
        }
    }

    println!("Restore command: {}", preview.restore_command);
    println!("Confirmation phrase: {}", preview.confirmation_phrase);
    println!("Dry run only. No files were written to CookedPCConsole.");
}

fn print_install_report_summary(report: &InstallReport) {
    println!("Install profile: {}", report.profile_name);
    println!("Status: {}", report.status);
    println!("CookedPCConsole: {}", report.cooked_root);
    println!(
        "Installed at: {}",
        report
            .installed_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "[not installed]".to_string())
    );
    println!(
        "Profile backup manifest: {}",
        report.profile_backup_manifest_path
    );
    println!(
        "Original backup manifest: {}",
        report.original_backup_manifest_path
    );
    println!(
        "Install manifest: {}",
        report
            .install_manifest_path
            .as_deref()
            .unwrap_or("[not written]")
    );
    println!(
        "Overwrite existing profile backup: {}",
        yes_no(report.overwrite_profile_backup)
    );
    if report.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &report.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning.message);
        }
    }
    if report.files.is_empty() {
        println!("Installed files: [none]");
    } else {
        println!("Installed files:");
        for file in &report.files {
            println!(
                "  {}: {} <- {}",
                file.kind, file.target_path, file.built_path
            );
        }
    }
    println!("Restore command: {}", report.restore_command);
    println!("Confirmation phrase: {}", report.confirmation_phrase);
}

fn print_restore_report_summary(report: &RestoreReport) {
    println!("Restore profile: {}", report.profile_name);
    println!("Status: {}", report.status);
    println!("Dry run: {}", yes_no(report.dry_run));
    println!("From originals: {}", yes_no(report.from_originals));
    println!("CookedPCConsole: {}", report.cooked_root);
    println!(
        "Restored at: {}",
        report
            .restored_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "[not restored]".to_string())
    );
    println!(
        "Install manifest: {}",
        report
            .install_manifest_path
            .as_deref()
            .unwrap_or("[not found]")
    );
    println!(
        "Profile backup manifest: {}",
        report.profile_backup_manifest_path
    );
    println!(
        "Original backup manifest: {}",
        report.original_backup_manifest_path
    );
    if report.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &report.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning.message);
        }
    }
    if report.files.is_empty() {
        println!("Restore files: [none]");
    } else {
        println!("Restore files:");
        for file in &report.files {
            println!(
                "  {}: {} -> {} [{}]",
                file.kind, file.backup_source_path, file.destination_path, file.backup_kind
            );
            println!(
                "    expected={} backup={} restored={}",
                file.expected_sha256,
                file.backup_sha256.as_deref().unwrap_or("[not available]"),
                file.actual_restored_sha256
                    .as_deref()
                    .unwrap_or("[not restored]")
            );
        }
    }
    println!("Restore command: {}", report.restore_command);
    println!("Confirmation phrase: {}", report.confirmation_phrase);
    if report.dry_run {
        println!("Dry run only. No files were written to CookedPCConsole.");
    }
}

fn prompt_install_confirmation(phrase: &str) -> Result<Option<String>> {
    println!("Type '{}' to continue, or press Enter to cancel.", phrase);
    print!("confirm> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim().to_string();
    if value.is_empty() {
        return Ok(None);
    }

    Ok(Some(value))
}

fn print_backup_result_summary(result: &BackupResult) {
    println!("Backup kind: {}", result.backup_kind);
    println!("Status: {}", result.status);
    if let Some(profile_name) = &result.profile_name {
        println!("Profile: {}", profile_name);
    }
    println!("Backup root: {}", result.backup_root);
    println!("Manifest: {}", result.manifest_path);
    println!("Created files: {}", result.created_count);
    println!("Existing files: {}", result.existing_count);
    println!("Verified files: {}", result.verified_count);
    if result.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &result.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if result.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &result.warnings {
            println!("  - {}", warning.message);
        }
    }
    if result.files.is_empty() {
        println!("Files: [none]");
    } else {
        println!("Files:");
        for file in &result.files {
            println!(
                "  {}: {} -> {} ({})",
                file.operation_kind, file.source_path, file.backup_path, file.status
            );
        }
    }
}

fn print_backup_verification_summary(result: &BackupVerificationResult) {
    println!("Backup kind: {}", result.backup_kind);
    println!("Status: {}", result.status);
    println!("Backup root: {}", result.backup_root);
    println!("Manifest: {}", result.manifest_path);
    println!("Tracked files: {}", result.tracked_file_count);
    println!("Missing files: {}", result.missing_file_count);
    println!("Mismatched files: {}", result.mismatched_file_count);
    println!("Untracked files: {}", result.untracked_file_count);
    if result.blockers.is_empty() {
        println!("Blockers: none");
    } else {
        println!("Blockers:");
        for blocker in &result.blockers {
            println!("  - {}", blocker.message);
        }
    }
    if result.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &result.warnings {
            println!("  - {}", warning.message);
        }
    }
    if result.files.is_empty() {
        println!("Files: [none]");
    } else {
        println!("Files:");
        for file in &result.files {
            println!(
                "  {}: {} ({})",
                file.target_relative_path, file.backup_path, file.status
            );
        }
    }
}

fn print_known_answer_summary(report: &KnownAnswerReport) {
    println!("Known-answer harness:");
    println!("Source: {}", report.source.path);
    println!("Target: {}", report.target.path);
    println!(
        "Expected: {}",
        report
            .expected
            .as_ref()
            .map(|value| value.path.as_str())
            .unwrap_or("[not provided]")
    );
    println!(
        "Generated output: {}",
        report
            .generated_output
            .as_ref()
            .map(|value| value.path.as_str())
            .or(report.output_plan.sandbox_output_path.as_deref())
            .unwrap_or("[not generated]")
    );
    println!(
        "Source identity: {}",
        report.source_identity.as_deref().unwrap_or("[not derived]")
    );
    println!(
        "Target identity: {}",
        report.target_identity.as_deref().unwrap_or("[not derived]")
    );
    println!(
        "Expected identity: {}",
        report
            .expected_identity
            .as_deref()
            .unwrap_or("[not derived]")
    );
    println!(
        "Target identity candidates: {}",
        if report.target_identity_candidates.is_empty() {
            "[none]".to_string()
        } else {
            report.target_identity_candidates.join(", ")
        }
    );
    println!(
        "Planned profile: {}",
        report
            .output_plan
            .profile_name
            .as_deref()
            .unwrap_or("[not resolved]")
    );
    println!(
        "Planned output filename: {}",
        report
            .output_plan
            .target_filename
            .as_deref()
            .unwrap_or("[not resolved]")
    );
    println!(
        "Sandbox output path: {}",
        report
            .output_plan
            .sandbox_output_path
            .as_deref()
            .unwrap_or("[not configured]")
    );
    println!(
        "Writer enabled: {}",
        yes_no(report.output_plan.generation_enabled)
    );
    println!(
        "Compared output body matches source: {}",
        yes_no_option(report.validation.source_body_matches_output_body)
    );
    println!(
        "Compared output exposes target identity: {}",
        yes_no_option(report.validation.target_identity_present)
    );
    println!(
        "Modified export refs detected: {}",
        yes_no_option(report.validation.modified_export_refs_detected)
    );
    if let Some(rebuild) = &report.generated_rebuild {
        println!(
            "Sandbox rebuild validation passed: {}",
            yes_no(rebuild.validation.passed)
        );
        println!(
            "Modified export indices: {}",
            format_indices(&rebuild.modified_export_indices)
        );
        println!("Rebuilt output filename: {}", rebuild.output_filename);
    }
    println!("Table counts:");
    println!(
        "  source:   {}",
        format_table_counts(&report.table_counts.source)
    );
    println!(
        "  target:   {}",
        format_table_counts(&report.table_counts.target)
    );
    if let Some(expected) = &report.table_counts.expected {
        println!("  expected: {}", format_table_counts(expected));
    }
    if let Some(output) = &report.table_counts.generated_output {
        println!("  output:   {}", format_table_counts(output));
    }
    if let Some(byte_comparison) = &report.validation.byte_comparison {
        println!(
            "Byte comparison: exact={} first_diff={} expected_len={} actual_len={}",
            yes_no(byte_comparison.exact_match),
            byte_comparison
                .first_difference_offset
                .map(|value| format!("0x{value:X}"))
                .unwrap_or_else(|| "[none]".to_string()),
            byte_comparison.expected_len,
            byte_comparison.actual_len,
        );
    } else {
        println!("Byte comparison: [not available]");
    }
    if report.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.warnings {
            println!("  - {}", warning);
        }
    }
}

fn print_sandbox_rebuild_summary(report: &SandboxRebuildReport) {
    println!("Sandbox rebuild:");
    println!("Source: {}", report.source_path);
    println!("Target: {}", report.target_path);
    println!("Output: {}", report.output_path);
    println!("Source identity: {}", report.source_identity);
    println!("Target identity: {}", report.target_identity);
    println!("Output filename: {}", report.output_filename);
    println!(
        "Appended target name: {}",
        report.appended_target_name.as_deref().unwrap_or("[no]")
    );
    println!("Header name delta: {}", report.name_delta);
    println!(
        "Modified export indices: {}",
        format_indices(&report.modified_export_indices)
    );
    println!("Validation passed: {}", yes_no(report.validation.passed));
    println!(
        "Filename matches target: {}",
        yes_no(report.validation.filename_matches_target)
    );
    println!(
        "Body equals source: {}",
        yes_no(report.validation.body_equals_source)
    );
    println!(
        "Target name present: {}",
        yes_no(report.validation.target_name_present)
    );
    println!(
        "Target export ref count: {}",
        report.validation.target_export_name_count
    );
    if report.validation.warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in &report.validation.warnings {
            println!("  - {}", warning);
        }
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let mut output = value
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    output.push('…');
    output
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn yes_no_option(value: Option<bool>) -> &'static str {
    match value {
        Some(value) => yes_no(value),
        None => "n/a",
    }
}

fn format_table_counts(snapshot: &TableCountSnapshot) -> String {
    format!(
        "names={} imports={} exports={} depends={} chunks={}",
        snapshot.name_count,
        snapshot.import_count,
        snapshot.export_count,
        snapshot
            .depends_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "[not parsed]".to_string()),
        snapshot
            .compressed_chunk_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "[not parsed]".to_string())
    )
}

fn format_indices(values: &[usize]) -> String {
    if values.is_empty() {
        return "[none]".to_string();
    }

    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn configured_cooked_dir_string() -> Result<Option<String>> {
    Ok(PathService::new(DatabaseService::for_current_user()?)
        .configured_cooked_dir()?
        .map(|value| value.display().to_string()))
}
