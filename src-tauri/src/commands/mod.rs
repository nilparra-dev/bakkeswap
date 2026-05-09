use anyhow::Result;
use bakkeswap_core::database::{
    CodeRedImportSource, DatabaseImportSummary, DatabaseImporter, DatabaseService,
    LocalFileIndexSummary, LocalFileIndexer, SearchEngine, SearchHit, SearchRequest,
};
use bakkeswap_core::domain::models::{
    AppStatus, BackupVerificationResult, GamePathValidation, InstallPreview, InstallReport,
    PlanBuildReport, RestoreReport, SwapPlan,
};
use bakkeswap_core::services::backups::PermanentOriginalBackupManager;
use bakkeswap_core::services::paths::ConfigSnapshot;
use bakkeswap_core::services::{
    BuildPlanRequest, BuildService, InstallExecutionRequest, InstallPreviewRequest,
    InstallerService, PathService, PlannerService, RestoreExecutionRequest, RestorePreviewRequest,
    RestoreService, StatusService,
};
use serde::Serialize;

type CommandResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize)]
pub struct RefreshDbResult {
    pub import_summary: DatabaseImportSummary,
    pub local_index_summary: Option<LocalFileIndexSummary>,
    pub warnings: Vec<String>,
    pub status: AppStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledSwapSummary {
    pub install_id: String,
    pub plan_id: String,
    pub profile_name: String,
    pub cooked_root: String,
    pub manifest_path: Option<String>,
    pub installed_at: String,
    pub restored_at: Option<String>,
    pub active: bool,
    pub dry_run_only: bool,
    pub plan_status: Option<String>,
    pub plan_path: Option<String>,
    pub target_product_id: Option<i64>,
    pub target_name: Option<String>,
    pub source_product_id: Option<i64>,
    pub source_name: Option<String>,
}

#[tauri::command]
pub async fn get_app_status() -> CommandResult<AppStatus> {
    run_with_database(|database| StatusService::new(database).current_status()).await
}

#[tauri::command]
pub async fn get_config() -> CommandResult<ConfigSnapshot> {
    run_with_database(|database| PathService::new(database).show_config()).await
}

#[tauri::command]
pub async fn set_game_path(path: String) -> CommandResult<GamePathValidation> {
    run_with_database(move |database| PathService::new(database).set_game_path(&path)).await
}

#[tauri::command]
pub async fn validate_game_path(path: String) -> CommandResult<GamePathValidation> {
    run_with_database(move |database| PathService::new(database).validate_game_path(&path)).await
}

#[tauri::command]
pub async fn import_codered(source: CodeRedImportSource) -> CommandResult<DatabaseImportSummary> {
    run_with_database(move |database| DatabaseImporter::new(database).import_codered(&source)).await
}

#[tauri::command]
pub async fn refresh_db() -> CommandResult<RefreshDbResult> {
    run_with_database(move |database| {
        let importer = DatabaseImporter::new(database.clone());
        let indexer = LocalFileIndexer::new(database.clone());
        let paths = PathService::new(database.clone());
        let status_service = StatusService::new(database);

        let import_summary = importer.refresh()?;
        let mut warnings = Vec::new();
        let local_index_summary = match paths.configured_cooked_dir()? {
            Some(cooked_dir) => Some(indexer.index_cooked_dir(&cooked_dir)?),
            None => {
                warnings.push(
                    "No CookedPCConsole path is configured yet, so local .upk indexing was skipped."
                        .to_string(),
                );
                None
            }
        };

        Ok(RefreshDbResult {
            import_summary,
            local_index_summary,
            warnings,
            status: status_service.current_status()?,
        })
    })
    .await
}

#[tauri::command]
pub async fn search_items(mut request: SearchRequest) -> CommandResult<Vec<SearchHit>> {
    request.limit = request.limit.clamp(1, 100);
    run_with_database(move |database| SearchEngine::new(database).search_products(&request)).await
}

#[tauri::command]
pub async fn create_plan(
    target_product_id: i64,
    source_product_id: i64,
) -> CommandResult<SwapPlan> {
    run_with_database(move |database| {
        PlannerService::new(database).create_plan(target_product_id, source_product_id)
    })
    .await
}

#[tauri::command]
pub async fn build_plan(request: BuildPlanRequest) -> CommandResult<PlanBuildReport> {
    run_with_database(move |database| BuildService::new(database).build_plan(&request)).await
}

#[tauri::command]
pub async fn install_preview(request: InstallPreviewRequest) -> CommandResult<InstallPreview> {
    run_with_database(move |database| InstallerService::new(database).preview_install(&request))
        .await
}

#[tauri::command]
pub async fn install_confirmed(request: InstallExecutionRequest) -> CommandResult<InstallReport> {
    run_with_database(move |database| InstallerService::new(database).install(&request)).await
}

#[tauri::command]
pub async fn restore_preview(request: RestorePreviewRequest) -> CommandResult<RestoreReport> {
    run_with_database(move |database| RestoreService::new(database).preview_restore(&request)).await
}

#[tauri::command]
pub async fn restore_confirmed(request: RestoreExecutionRequest) -> CommandResult<RestoreReport> {
    run_with_database(move |database| RestoreService::new(database).restore(&request)).await
}

#[tauri::command]
pub async fn backup_originals_status() -> CommandResult<BackupVerificationResult> {
    run_with_database(move |database| PermanentOriginalBackupManager::new(database).status()).await
}

#[tauri::command]
pub async fn backup_originals_verify() -> CommandResult<BackupVerificationResult> {
    run_with_database(move |database| PermanentOriginalBackupManager::new(database).verify()).await
}

#[tauri::command]
pub async fn list_installed_swaps() -> CommandResult<Vec<InstalledSwapSummary>> {
    run_with_database(load_installed_swap_summaries).await
}

async fn run_with_database<T, F>(task: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(DatabaseService) -> Result<T> + Send + 'static,
{
    run_blocking(move || {
        let database = DatabaseService::for_current_user().map_err(command_error)?;
        task(database).map_err(command_error)
    })
    .await
}

async fn run_blocking<T, F>(task: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("background task failed: {error}"))?
}

fn command_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn load_installed_swap_summaries(database: DatabaseService) -> Result<Vec<InstalledSwapSummary>> {
    let connection = database.connect()?;
    let mut statement = connection.prepare(
        "SELECT i.install_id,
                i.plan_id,
                i.profile_name,
                i.cooked_root,
                i.manifest_path,
                i.installed_at,
                i.restored_at,
                i.active,
                i.dry_run_only,
                sp.status,
                sp.plan_path,
                sp.target_product_id,
                target.name,
                sp.source_product_id,
                source.name
         FROM installed_swaps i
         LEFT JOIN swap_plans sp ON sp.plan_id = i.plan_id
         LEFT JOIN products target ON target.product_id = sp.target_product_id
         LEFT JOIN products source ON source.product_id = sp.source_product_id
         ORDER BY i.active DESC, i.installed_at DESC",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(InstalledSwapSummary {
            install_id: row.get(0)?,
            plan_id: row.get(1)?,
            profile_name: row.get(2)?,
            cooked_root: row.get(3)?,
            manifest_path: row.get(4)?,
            installed_at: row.get(5)?,
            restored_at: row.get(6)?,
            active: row.get::<_, i64>(7)? != 0,
            dry_run_only: row.get::<_, i64>(8)? != 0,
            plan_status: row.get(9)?,
            plan_path: row.get(10)?,
            target_product_id: row.get(11)?,
            target_name: row.get(12)?,
            source_product_id: row.get(13)?,
            source_name: row.get(14)?,
        })
    })?;

    let mut summaries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    summaries.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.installed_at.cmp(&left.installed_at))
    });
    Ok(summaries)
}
