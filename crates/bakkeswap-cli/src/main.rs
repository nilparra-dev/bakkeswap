use std::path::PathBuf;

use anyhow::Result;
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
        Command::Selftest => print_stub("selftest"),
        Command::Config { command } => match command {
            ConfigCommand::SetGamePath { path } => print_stub_with_value("config set-game-path", path.display().to_string()),
            ConfigCommand::Show => print_stub("config show"),
            ConfigCommand::Validate => print_stub("config validate"),
        },
        Command::Db { command } => match command {
            DbCommand::ImportCodered { folder } => print_stub_with_value("db import-codered", folder.display().to_string()),
            DbCommand::Refresh => print_stub("db refresh"),
        },
        Command::Search(args) => print_stub_with_value("search", args.query),
        Command::Plan(args) => print_stub_with_value("plan", format!("target={} source={}", args.target, args.source)),
        Command::Build(args) => print_stub_with_value("build", args.plan.display().to_string()),
        Command::Install(args) => print_stub_with_value(
            if args.dry_run { "install --dry-run" } else { "install" },
            args.plan.display().to_string(),
        ),
        Command::Restore(args) => print_stub_with_value("restore", args.profile),
        Command::Status => print_stub("status"),
        Command::Backup { command } => match command {
            BackupCommand::Originals { command } => match command {
                BackupOriginalsCommand::Status => print_stub("backup originals status"),
                BackupOriginalsCommand::Verify => print_stub("backup originals verify"),
            },
        },
    }
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
