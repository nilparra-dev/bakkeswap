import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  AppStatus,
  BackupVerificationResult,
  ConfigSnapshot,
  DatabaseImportSummary,
  GamePathValidation,
  InstallPreview,
  InstallReport,
  InstalledSwapSummary,
  PlanBuildReport,
  RefreshDbResult,
  RestoreReport,
  SearchHit,
  SwapPlan,
} from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriAvailable(): boolean {
  return typeof window !== "undefined" && typeof window.__TAURI_INTERNALS__ !== "undefined";
}

function ensureTauri(): void {
  if (!isTauriAvailable()) {
    throw new Error(
      "Desktop backend unavailable. Run the app with 'npm run tauri:dev' for full functionality.",
    );
  }
}

async function invokeCommand<T>(command: string, payload?: Record<string, unknown>): Promise<T> {
  ensureTauri();
  return invoke<T>(command, payload);
}

export async function pickFolder(title: string): Promise<string | null> {
  ensureTauri();
  const selection = await open({ directory: true, multiple: false, title });
  if (Array.isArray(selection)) {
    return selection[0] ?? null;
  }
  return selection ?? null;
}

export async function getAppStatus(): Promise<AppStatus> {
  return invokeCommand<AppStatus>("get_app_status");
}

export async function getConfig(): Promise<ConfigSnapshot> {
  return invokeCommand<ConfigSnapshot>("get_config");
}

export async function validateGamePath(path: string): Promise<GamePathValidation> {
  return invokeCommand<GamePathValidation>("validate_game_path", { path });
}

export async function setGamePath(path: string): Promise<GamePathValidation> {
  return invokeCommand<GamePathValidation>("set_game_path", { path });
}

export async function importCodeRed(folder: string): Promise<DatabaseImportSummary> {
  return invokeCommand<DatabaseImportSummary>("import_codered", {
    source: { folder },
  });
}

export async function refreshDb(): Promise<RefreshDbResult> {
  return invokeCommand<RefreshDbResult>("refresh_db");
}

export async function searchItems(query: string, limit: number): Promise<SearchHit[]> {
  return invokeCommand<SearchHit[]>("search_items", {
    request: { query, limit },
  });
}

export async function createPlan(
  targetProductId: number,
  sourceProductId: number,
): Promise<SwapPlan> {
  return invokeCommand<SwapPlan>("create_plan", {
    target_product_id: targetProductId,
    source_product_id: sourceProductId,
  });
}

export async function buildPlan(planPath: string): Promise<PlanBuildReport> {
  return invokeCommand<PlanBuildReport>("build_plan", {
    request: {
      plan_path: planPath,
      output_root: null,
      create_dir: true,
    },
  });
}

export async function installPreview(
  planPath: string,
  buildReport: PlanBuildReport | null,
): Promise<InstallPreview> {
  return invokeCommand<InstallPreview>("install_preview", {
    request: {
      plan_path: planPath,
      build_report: buildReport,
      configured_cooked_root: null,
      workspace_root: null,
      dry_run: true,
    },
  });
}

export async function installConfirmed(
  planPath: string,
  buildReport: PlanBuildReport | null,
  confirmation: string,
  overwriteProfileBackup: boolean,
): Promise<InstallReport> {
  return invokeCommand<InstallReport>("install_confirmed", {
    request: {
      plan_path: planPath,
      build_report: buildReport,
      configured_cooked_root: null,
      workspace_root: null,
      confirmation,
      overwrite_profile_backup: overwriteProfileBackup,
    },
  });
}

export async function listInstalledSwaps(): Promise<InstalledSwapSummary[]> {
  return invokeCommand<InstalledSwapSummary[]>("list_installed_swaps");
}

export async function restorePreview(
  profileName: string,
  fromOriginals: boolean,
): Promise<RestoreReport> {
  return invokeCommand<RestoreReport>("restore_preview", {
    request: {
      profile_name: profileName,
      from_originals: fromOriginals,
      configured_cooked_root: null,
      workspace_root: null,
    },
  });
}

export async function restoreConfirmed(
  profileName: string,
  fromOriginals: boolean,
  confirmation: string,
): Promise<RestoreReport> {
  return invokeCommand<RestoreReport>("restore_confirmed", {
    request: {
      profile_name: profileName,
      from_originals: fromOriginals,
      confirmation,
      configured_cooked_root: null,
      workspace_root: null,
    },
  });
}

export async function backupOriginalsStatus(): Promise<BackupVerificationResult> {
  return invokeCommand<BackupVerificationResult>("backup_originals_status");
}

export async function backupOriginalsVerify(): Promise<BackupVerificationResult> {
  return invokeCommand<BackupVerificationResult>("backup_originals_verify");
}