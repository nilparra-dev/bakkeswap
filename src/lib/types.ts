export type PageId =
  | "home"
  | "game-folder"
  | "database"
  | "quick-swap"
  | "install-preview"
  | "active-swaps"
  | "backups"
  | "logs";

export interface UiNotice {
  code?: string;
  message: string;
}

export interface GamePathValidation {
  input_path: string;
  normalized_cooked_dir: string | null;
  input_kind: string | null;
  is_valid: boolean;
  input_exists: boolean;
  cooked_exists: boolean;
  upk_count: number;
  sample_upks: string[];
  warnings: string[];
  errors: string[];
}

export interface ConfigSnapshot {
  game_path_input: string | null;
  cooked_dir: string | null;
  codered_dumps_dir: string | null;
  app_home: string;
  database_path: string;
  validation: GamePathValidation | null;
}

export interface AppStatus {
  configured_game_path: string | null;
  configured_cooked_dir: string | null;
  configured_codered_dumps_dir: string | null;
  database_ready: boolean;
  local_files_indexed: boolean;
  local_files_count: number;
  product_count: number;
  title_count: number;
  active_swap_count: number;
  original_backup_count: number;
  profile_backup_count: number;
}

export type SearchKind = "product" | "title";

export interface SearchHit {
  kind: SearchKind;
  id: string;
  name: string;
  slot: string | null;
  quality: string | null;
  product_asset_package: string | null;
  product_thumbnail_package: string | null;
  swappable: boolean;
  note: string | null;
}

export interface PlannedProduct {
  id: number;
  name: string;
  slot: string | null;
  slot_id: number | null;
  quality: string | null;
  paintable: boolean;
  visual_upk: string | null;
  thumb_upk: string | null;
  visual_asset: string | null;
  thumbnail_asset: string | null;
}

export interface CompatibilityCheck {
  same_slot: boolean;
}

export interface SwapOperation {
  kind: string;
  enabled: boolean;
  source_filename: string | null;
  target_filename: string | null;
  source_identity: string | null;
  target_identity: string | null;
  source_path: string | null;
  target_path: string | null;
  source_sha256: string | null;
  target_sha256: string | null;
  backup_path: string | null;
  output_path: string | null;
}

export interface PlanNotice {
  code: string;
  message: string;
}

export interface BuildValidation {
  output_exists: boolean;
  filename_matches_target: boolean;
  output_parses: boolean;
  output_decrypts_tables: boolean;
  output_decompresses: boolean;
  body_equals_source: boolean;
  target_name_present: boolean;
  target_export_name_count: number;
  modified_export_indices: number[];
  output_sha256: string | null;
  source_body_sha256: string | null;
  output_body_sha256: string | null;
  warnings: string[];
  passed: boolean;
}

export interface PlanBuildReport {
  build_id: string;
  profile_name: string;
  status: string;
  build_root: string;
  visual_output_path: string | null;
  visual_validation: BuildValidation | null;
  thumbnail_output_path: string | null;
  thumbnail_validation: BuildValidation | null;
  warnings: PlanNotice[];
  blockers: PlanNotice[];
  no_install_performed: boolean;
  created_at: string;
}

export interface BackupPreview {
  backup_kind: string;
  operation_kind: string;
  target_relative_path: string;
  backup_path: string;
  exists: boolean;
  would_create: boolean;
  status: string;
  warnings: string[];
}

export interface InstallPreviewFile {
  kind: string;
  target_filename: string;
  target_path: string;
  built_path: string;
  destination_exists: boolean;
  built_exists: boolean;
  would_overwrite: boolean;
  current_destination_sha256: string | null;
  planned_original_sha256: string | null;
  built_output_sha256: string | null;
  current_matches_planned_original: boolean | null;
  current_matches_built_output: boolean | null;
}

export interface InstallPreview {
  plan_path: string;
  profile_name: string;
  status: string;
  configured_cooked_root: string;
  workspace_root: string;
  build_root: string;
  files: InstallPreviewFile[];
  profile_backups: BackupPreview[];
  permanent_original_backups: BackupPreview[];
  original_backup_manifest_path: string;
  restore_command: string;
  confirmation_phrase: string;
  dry_run_only: boolean;
  no_files_written: boolean;
  warnings: PlanNotice[];
  blockers: PlanNotice[];
}

export interface InstalledFileRecord {
  kind: string;
  relative_path: string;
  target_path: string;
  built_path: string;
  profile_backup_path: string;
  original_backup_path: string;
  original_sha256: string | null;
  built_sha256: string | null;
  installed_sha256: string | null;
}

export interface InstallReport {
  plan_id: string;
  plan_path: string;
  profile_name: string;
  status: string;
  installed: boolean;
  installed_at: string | null;
  restored_at: string | null;
  cooked_root: string;
  profile_backup_manifest_path: string;
  original_backup_manifest_path: string;
  install_manifest_path: string | null;
  overwrite_profile_backup: boolean;
  files: InstalledFileRecord[];
  warnings: PlanNotice[];
  blockers: PlanNotice[];
  restore_command: string;
  confirmation_phrase: string;
}

export interface SwapPlan {
  plan_id: string;
  schema_version: number;
  created_at: string;
  profile_name: string;
  offline_only: boolean;
  database_path: string | null;
  configured_cooked_root: string | null;
  status: string;
  target_product: PlannedProduct;
  source_product: PlannedProduct;
  compatibility: CompatibilityCheck;
  operations: SwapOperation[];
  warnings: PlanNotice[];
  build_blockers: PlanNotice[];
  last_build: PlanBuildReport | null;
  install_status: string;
  last_install: InstallReport | null;
  rollback_notes: string[];
  plan_path: string;
}

export interface RestoreFileRecord {
  kind: string;
  relative_path: string;
  backup_kind: string;
  backup_source_path: string;
  destination_path: string;
  destination_exists: boolean;
  expected_sha256: string;
  backup_sha256: string | null;
  backup_hash_matches_expected: boolean | null;
  actual_restored_sha256: string | null;
}

export interface RestoreReport {
  plan_id: string | null;
  plan_path: string | null;
  profile_name: string;
  status: string;
  dry_run: boolean;
  from_originals: boolean;
  restored: boolean;
  restored_at: string | null;
  cooked_root: string;
  install_manifest_path: string | null;
  profile_backup_manifest_path: string;
  original_backup_manifest_path: string;
  files: RestoreFileRecord[];
  warnings: PlanNotice[];
  blockers: PlanNotice[];
  restore_command: string;
  confirmation_phrase: string;
}

export interface DatabaseImportSummary {
  source_dir: string;
  imported_products: number;
  imported_slots: number;
  imported_paints: number;
  imported_titles: number;
}

export interface LocalFileIndexSummary {
  cooked_root: string;
  indexed_files: number;
}

export interface RefreshDbResult {
  import_summary: DatabaseImportSummary;
  local_index_summary: LocalFileIndexSummary | null;
  warnings: string[];
  status: AppStatus;
}

export interface BackupFileResult {
  backup_kind: string;
  operation_kind: string;
  target_relative_path: string;
  source_path: string;
  backup_path: string;
  sha256: string | null;
  size_bytes: number | null;
  status: string;
  warnings: string[];
}

export interface BackupVerificationResult {
  backup_kind: string;
  status: string;
  backup_root: string;
  manifest_path: string;
  tracked_file_count: number;
  missing_file_count: number;
  mismatched_file_count: number;
  untracked_file_count: number;
  files: BackupFileResult[];
  warnings: PlanNotice[];
  blockers: PlanNotice[];
}

export interface InstalledSwapSummary {
  install_id: string;
  plan_id: string;
  profile_name: string;
  cooked_root: string;
  manifest_path: string | null;
  installed_at: string;
  restored_at: string | null;
  active: boolean;
  dry_run_only: boolean;
  plan_status: string | null;
  plan_path: string | null;
  target_product_id: number | null;
  target_name: string | null;
  source_product_id: number | null;
  source_name: string | null;
}

export interface CommandLog {
  id: string;
  at: string;
  kind: "started" | "success" | "error";
  command: string;
  detail: string;
}

export interface SearchPaneState {
  query: string;
  loading: boolean;
  error: string | null;
  results: SearchHit[];
  selected: SearchHit | null;
}

export interface SetupPaneState {
  game_path_input: string;
  validating: boolean;
  saving: boolean;
  validation: GamePathValidation | null;
  error: string | null;
}

export interface DatabasePaneState {
  import_folder_input: string;
  importing: boolean;
  refreshing: boolean;
  last_import_summary: DatabaseImportSummary | null;
  last_refresh_result: RefreshDbResult | null;
  error: string | null;
}

export interface QuickSwapPaneState {
  target: SearchPaneState;
  source: SearchPaneState;
  creating_plan: boolean;
  building: boolean;
  previewing_install: boolean;
  installing: boolean;
  overwrite_profile_backup: boolean;
  install_confirmation: string;
  plan: SwapPlan | null;
  build_report: PlanBuildReport | null;
  install_preview: InstallPreview | null;
  install_report: InstallReport | null;
  error: string | null;
}

export interface RestorePaneState {
  installed_swaps: InstalledSwapSummary[];
  loading: boolean;
  selected_profile_name: string | null;
  previewing: boolean;
  restoring: boolean;
  from_originals: boolean;
  confirmation: string;
  preview: RestoreReport | null;
  report: RestoreReport | null;
  error: string | null;
}

export interface BackupsPaneState {
  loading: boolean;
  verifying: boolean;
  status: BackupVerificationResult | null;
  error: string | null;
}

export interface AppUiState {
  tauri_available: boolean;
  bootstrap_loading: boolean;
  runtime_error: string | null;
  active_page: PageId;
  app_status: AppStatus | null;
  config: ConfigSnapshot | null;
  setup: SetupPaneState;
  database: DatabasePaneState;
  quick_swap: QuickSwapPaneState;
  restore: RestorePaneState;
  backups: BackupsPaneState;
  logs: CommandLog[];
}