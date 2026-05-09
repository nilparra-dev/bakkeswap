use std::path::PathBuf;

use anyhow::Result;
use bakkeswap_core::database::{
    CodeRedImportSource, DatabaseImporter, DatabaseService, LocalFileIndexer, SearchEngine,
    SearchKind, SearchRequest,
};
use bakkeswap_core::services::{PathService, StatusService};
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

#[derive(Debug, Args)]
struct SearchArgs {
    query: String,
    #[arg(long, default_value_t = 25)]
    limit: usize,
    #[arg(long, default_value_t = false)]
    json: bool,
}

#[derive(Debug, Args)]
struct PlanArgs {
    #[arg(long)]
    target: i64,
    #[arg(long)]
    source: i64,
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[arg(long)]
    plan: PathBuf,
}

#[derive(Debug, Args)]
struct InstallArgs {
    #[arg(long)]
    plan: PathBuf,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct RestoreArgs {
    #[arg(long)]
    profile: String,
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    Originals {
        #[command(subcommand)]
        command: BackupOriginalsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BackupOriginalsCommand {
    Status,
    Verify,
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
        Command::Plan(args) => print_stub_with_value(
            "plan",
            format!("target={} source={}", args.target, args.source),
        ),
        Command::Build(args) => print_stub_with_value("build", args.plan.display().to_string()),
        Command::Install(args) => print_stub_with_value(
            if args.dry_run {
                "install --dry-run"
            } else {
                "install"
            },
            args.plan.display().to_string(),
        ),
        Command::Restore(args) => print_stub_with_value("restore", args.profile),
        Command::Status => command_status(),
        Command::Backup { command } => match command {
            BackupCommand::Originals { command } => match command {
                BackupOriginalsCommand::Status => print_stub("backup originals status"),
                BackupOriginalsCommand::Verify => print_stub("backup originals verify"),
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

fn command_status() -> Result<()> {
    let status = StatusService::new(DatabaseService::for_current_user()?).current_status()?;
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
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

fn print_stub(command: &str) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "command": command,
            "status": "skeleton-only",
            "message": "Rust CLI contract created; implementation not ported yet."
        }))?
    );
    Ok(())
}

fn print_stub_with_value(command: &str, value: String) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "command": command,
            "value": value,
            "status": "skeleton-only",
            "message": "Rust CLI contract created; implementation not ported yet."
        }))?
    );
    Ok(())
}
