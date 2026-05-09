import { writable } from "svelte/store";

import {
  backupOriginalsStatus,
  backupOriginalsVerify,
  buildPlan,
  createPlan,
  getAppStatus,
  getConfig,
  importCodeRed,
  installConfirmed,
  installPreview,
  isTauriAvailable,
  listInstalledSwaps,
  pickFolder,
  refreshDb,
  restoreConfirmed,
  restorePreview,
  searchItems,
  setGamePath,
  validateGamePath,
} from "./api";
import type {
  AppUiState,
  BackupVerificationResult,
  CommandLog,
  ConfigSnapshot,
  GamePathValidation,
  InstalledSwapSummary,
  PageId,
  PlanBuildReport,
  RefreshDbResult,
  SearchHit,
  SearchPaneState,
  UiNotice,
} from "./types";

const SEARCH_LIMIT = 50;
const SEARCH_DEBOUNCE_MS = 250;
const LOG_LIMIT = 40;

type SearchRole = "target" | "source";

function emptySearchPane(): SearchPaneState {
  return {
    query: "",
    loading: false,
    error: null,
    results: [],
    selected: null,
  };
}

function initialState(): AppUiState {
  return {
    tauri_available: isTauriAvailable(),
    bootstrap_loading: true,
    runtime_error: null,
    active_page: "home",
    app_status: null,
    config: null,
    setup: {
      game_path_input: "",
      validating: false,
      saving: false,
      validation: null,
      error: null,
    },
    database: {
      import_folder_input: "",
      importing: false,
      refreshing: false,
      last_import_summary: null,
      last_refresh_result: null,
      error: null,
    },
    quick_swap: {
      target: emptySearchPane(),
      source: emptySearchPane(),
      creating_plan: false,
      building: false,
      previewing_install: false,
      installing: false,
      overwrite_profile_backup: false,
      install_confirmation: "",
      plan: null,
      build_report: null,
      install_preview: null,
      install_report: null,
      error: null,
    },
    restore: {
      installed_swaps: [],
      loading: false,
      selected_profile_name: null,
      previewing: false,
      restoring: false,
      from_originals: false,
      confirmation: "",
      preview: null,
      report: null,
      error: null,
    },
    backups: {
      loading: false,
      verifying: false,
      status: null,
      error: null,
    },
    logs: [],
  };
}

function asErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }
  return String(error);
}

function clearSwapWorkflow(state: AppUiState): void {
  state.quick_swap.plan = null;
  state.quick_swap.build_report = null;
  state.quick_swap.install_preview = null;
  state.quick_swap.install_report = null;
  state.quick_swap.install_confirmation = "";
  state.quick_swap.overwrite_profile_backup = false;
  state.quick_swap.error = null;
}

function summaryFromBackupStatus(status: BackupVerificationResult): string {
  return `${status.tracked_file_count} tracked / ${status.missing_file_count} missing`;
}

function createAppStore() {
  const { subscribe, update } = writable<AppUiState>(initialState());
  let snapshot = initialState();
  subscribe((value) => {
    snapshot = value;
  });

  const searchTokens: Record<SearchRole, number> = {
    target: 0,
    source: 0,
  };
  const searchTimers: Partial<Record<SearchRole, number>> = {};

  function mutate(mutator: (draft: AppUiState) => void): void {
    update((state) => {
      const next = structuredClone(state);
      mutator(next);
      return next;
    });
  }

  function pushLog(kind: CommandLog["kind"], command: string, detail: string): void {
    mutate((state) => {
      state.logs.unshift({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        at: new Date().toISOString(),
        kind,
        command,
        detail,
      });
      state.logs = state.logs.slice(0, LOG_LIMIT);
    });
  }

  async function runLogged<T>(
    command: string,
    action: () => Promise<T>,
    detail?: (result: T) => string,
  ): Promise<T> {
    pushLog("started", command, "running");
    try {
      const result = await action();
      pushLog("success", command, detail ? detail(result) : "completed");
      return result;
    } catch (error) {
      pushLog("error", command, asErrorMessage(error));
      throw error;
    }
  }

  async function refreshOverview(syncInputs = false): Promise<void> {
    const results = await Promise.allSettled([
      runLogged("get_app_status", getAppStatus, (status) => {
        return `${status.product_count} products / ${status.local_files_count} local .upk files`;
      }),
      runLogged("get_config", getConfig, (config) => config.cooked_dir ?? "no game folder configured"),
      runLogged("list_installed_swaps", listInstalledSwaps, (swaps) => `${swaps.length} install records`),
      runLogged("backup_originals_status", backupOriginalsStatus, summaryFromBackupStatus),
    ]);

    mutate((state) => {
      const errors: string[] = [];
      const [statusResult, configResult, swapsResult, backupResult] = results;

      if (statusResult.status === "fulfilled") {
        state.app_status = statusResult.value;
      } else {
        errors.push(`Status: ${asErrorMessage(statusResult.reason)}`);
      }

      if (configResult.status === "fulfilled") {
        const config: ConfigSnapshot = configResult.value;
        state.config = config;
        state.setup.validation = config.validation;
        if (syncInputs || state.setup.game_path_input.trim().length === 0) {
          state.setup.game_path_input = config.game_path_input ?? config.cooked_dir ?? "";
        }
        if (syncInputs || state.database.import_folder_input.trim().length === 0) {
          state.database.import_folder_input = config.codered_dumps_dir ?? "";
        }
      } else {
        errors.push(`Config: ${asErrorMessage(configResult.reason)}`);
      }

      if (swapsResult.status === "fulfilled") {
        const swaps: InstalledSwapSummary[] = swapsResult.value;
        state.restore.installed_swaps = swaps;
        if (
          state.restore.selected_profile_name &&
          !swaps.some((swap) => swap.profile_name === state.restore.selected_profile_name)
        ) {
          state.restore.selected_profile_name = null;
          state.restore.preview = null;
          state.restore.report = null;
          state.restore.confirmation = "";
        }
      } else {
        errors.push(`Installed swaps: ${asErrorMessage(swapsResult.reason)}`);
      }

      if (backupResult.status === "fulfilled") {
        state.backups.status = backupResult.value;
      } else {
        errors.push(`Backups: ${asErrorMessage(backupResult.reason)}`);
      }

      state.bootstrap_loading = false;
      state.runtime_error = errors.length > 0 ? errors.join(" | ") : null;
    });
  }

  async function load(): Promise<void> {
    if (!isTauriAvailable()) {
      mutate((state) => {
        state.tauri_available = false;
        state.bootstrap_loading = false;
        state.runtime_error =
          "Desktop backend unavailable. Run this app with 'npm run tauri:dev' to enable the Rust services.";
      });
      return;
    }

    mutate((state) => {
      state.tauri_available = true;
      state.bootstrap_loading = true;
      state.runtime_error = null;
    });
    await refreshOverview(true);
  }

  async function browseFolderFor(kind: "game" | "codered"): Promise<void> {
    try {
      const folder = await runLogged("dialog.open", () =>
        pickFolder(kind === "game" ? "Select Rocket League folder" : "Select CodeRed dumps folder"),
      );
      if (!folder) {
        return;
      }
      mutate((state) => {
        if (kind === "game") {
          state.setup.game_path_input = folder;
          state.setup.validation = null;
          state.setup.error = null;
        } else {
          state.database.import_folder_input = folder;
          state.database.last_import_summary = null;
          state.database.error = null;
        }
      });
    } catch (error) {
      const message = asErrorMessage(error);
      mutate((state) => {
        if (kind === "game") {
          state.setup.error = message;
        } else {
          state.database.error = message;
        }
      });
    }
  }

  function setActivePage(page: PageId): void {
    mutate((state) => {
      state.active_page = page;
    });
  }

  function setGamePathInput(value: string): void {
    mutate((state) => {
      state.setup.game_path_input = value;
      state.setup.validation = null;
      state.setup.error = null;
    });
  }

  function setImportFolderInput(value: string): void {
    mutate((state) => {
      state.database.import_folder_input = value;
      state.database.last_import_summary = null;
      state.database.error = null;
    });
  }

  async function validateCurrentGamePath(): Promise<void> {
    const path = snapshot.setup.game_path_input.trim();
    mutate((state) => {
      state.setup.validating = true;
      state.setup.error = null;
    });
    try {
      const validation: GamePathValidation = await runLogged(
        "validate_game_path",
        () => validateGamePath(path),
        (result) => `${result.upk_count} .upk files scanned`,
      );
      mutate((state) => {
        state.setup.validation = validation;
      });
    } catch (error) {
      mutate((state) => {
        state.setup.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.setup.validating = false;
      });
    }
  }

  async function saveCurrentGamePath(): Promise<void> {
    const path = snapshot.setup.game_path_input.trim();
    mutate((state) => {
      state.setup.saving = true;
      state.setup.error = null;
    });
    try {
      const validation = await runLogged("set_game_path", () => setGamePath(path), (result) => {
        return result.normalized_cooked_dir ?? result.input_path;
      });
      mutate((state) => {
        state.setup.validation = validation;
      });
      await refreshOverview(true);
    } catch (error) {
      mutate((state) => {
        state.setup.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.setup.saving = false;
      });
    }
  }

  async function importCurrentCodeRedFolder(): Promise<void> {
    const folder = snapshot.database.import_folder_input.trim();
    mutate((state) => {
      state.database.importing = true;
      state.database.error = null;
    });
    try {
      const summary = await runLogged("import_codered", () => importCodeRed(folder), (result) => {
        return `${result.imported_products} products / ${result.imported_titles} titles imported`;
      });
      mutate((state) => {
        state.database.last_import_summary = summary;
      });
      await refreshOverview(true);
    } catch (error) {
      mutate((state) => {
        state.database.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.database.importing = false;
      });
    }
  }

  async function refreshDatabase(): Promise<void> {
    mutate((state) => {
      state.database.refreshing = true;
      state.database.error = null;
    });
    try {
      const result: RefreshDbResult = await runLogged("refresh_db", refreshDb, (summary) => {
        const localIndexed = summary.local_index_summary?.indexed_files ?? 0;
        return `${summary.import_summary.imported_products} products refreshed / ${localIndexed} local .upk indexed`;
      });
      mutate((state) => {
        state.database.last_refresh_result = result;
        state.app_status = result.status;
      });
      await refreshOverview(false);
    } catch (error) {
      mutate((state) => {
        state.database.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.database.refreshing = false;
      });
    }
  }

  function setSearchQuery(role: SearchRole, value: string): void {
    mutate((state) => {
      state.quick_swap[role].query = value;
      state.quick_swap[role].error = null;
      if (value.trim().length === 0) {
        state.quick_swap[role].results = [];
        state.quick_swap[role].loading = false;
      }
    });

    if (searchTimers[role]) {
      window.clearTimeout(searchTimers[role]);
    }

    searchTokens[role] += 1;
    const token = searchTokens[role];
    const query = value.trim();
    if (query.length === 0) {
      return;
    }

    searchTimers[role] = window.setTimeout(() => {
      void executeSearch(role, query, token);
    }, SEARCH_DEBOUNCE_MS);
  }

  async function executeSearch(role: SearchRole, query: string, token: number): Promise<void> {
    mutate((state) => {
      state.quick_swap[role].loading = true;
    });

    try {
      const results: SearchHit[] = await runLogged(
        "search_items",
        () => searchItems(query, SEARCH_LIMIT),
        (hits) => `${hits.length} ${role} results`,
      );
      if (searchTokens[role] !== token || snapshot.quick_swap[role].query.trim() !== query) {
        return;
      }
      mutate((state) => {
        state.quick_swap[role].results = results;
        state.quick_swap[role].loading = false;
      });
    } catch (error) {
      if (searchTokens[role] !== token) {
        return;
      }
      mutate((state) => {
        state.quick_swap[role].loading = false;
        state.quick_swap[role].error = asErrorMessage(error);
      });
    }
  }

  function selectSearchHit(role: SearchRole, hit: SearchHit): void {
    mutate((state) => {
      const current = state.quick_swap[role].selected;
      state.quick_swap[role].selected = hit;
      if (!current || current.id !== hit.id) {
        clearSwapWorkflow(state);
      }
    });
  }

  async function createCurrentPlan(): Promise<void> {
    const targetId = Number(snapshot.quick_swap.target.selected?.id ?? Number.NaN);
    const sourceId = Number(snapshot.quick_swap.source.selected?.id ?? Number.NaN);
    mutate((state) => {
      state.quick_swap.creating_plan = true;
      state.quick_swap.error = null;
      state.quick_swap.plan = null;
      state.quick_swap.build_report = null;
      state.quick_swap.install_preview = null;
      state.quick_swap.install_report = null;
      state.quick_swap.install_confirmation = "";
    });
    try {
      const plan = await runLogged("create_plan", () => createPlan(targetId, sourceId), (result) => {
        return `${result.profile_name} planned`;
      });
      mutate((state) => {
        state.quick_swap.plan = plan;
        state.quick_swap.build_report = plan.last_build;
      });
    } catch (error) {
      mutate((state) => {
        state.quick_swap.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.quick_swap.creating_plan = false;
      });
    }
  }

  async function buildCurrentPlan(): Promise<void> {
    const planPath = snapshot.quick_swap.plan?.plan_path;
    if (!planPath) {
      return;
    }
    mutate((state) => {
      state.quick_swap.building = true;
      state.quick_swap.error = null;
    });
    try {
      const report: PlanBuildReport = await runLogged("build_plan", () => buildPlan(planPath), (result) => {
        return `${result.status} at ${result.build_root}`;
      });
      mutate((state) => {
        state.quick_swap.build_report = report;
      });
    } catch (error) {
      mutate((state) => {
        state.quick_swap.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.quick_swap.building = false;
      });
    }
  }

  async function previewCurrentInstall(): Promise<void> {
    const planPath = snapshot.quick_swap.plan?.plan_path;
    if (!planPath) {
      return;
    }
    mutate((state) => {
      state.quick_swap.previewing_install = true;
      state.quick_swap.error = null;
    });
    try {
      const preview = await runLogged(
        "install_preview",
        () => installPreview(planPath, snapshot.quick_swap.build_report),
        (result) => `${result.status} for ${result.profile_name}`,
      );
      mutate((state) => {
        state.quick_swap.install_preview = preview;
        state.active_page = "install-preview";
      });
    } catch (error) {
      mutate((state) => {
        state.quick_swap.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.quick_swap.previewing_install = false;
      });
    }
  }

  function setInstallConfirmation(value: string): void {
    mutate((state) => {
      state.quick_swap.install_confirmation = value;
    });
  }

  function setOverwriteProfileBackup(value: boolean): void {
    mutate((state) => {
      state.quick_swap.overwrite_profile_backup = value;
    });
  }

  async function installCurrentPlan(): Promise<void> {
    const planPath = snapshot.quick_swap.plan?.plan_path;
    if (!planPath) {
      return;
    }
    mutate((state) => {
      state.quick_swap.installing = true;
      state.quick_swap.error = null;
    });
    try {
      const report = await runLogged(
        "install_confirmed",
        () =>
          installConfirmed(
            planPath,
            snapshot.quick_swap.build_report,
            snapshot.quick_swap.install_confirmation,
            snapshot.quick_swap.overwrite_profile_backup,
          ),
        (result) => `${result.status} for ${result.profile_name}`,
      );
      mutate((state) => {
        state.quick_swap.install_report = report;
        state.active_page = report.installed ? "active-swaps" : "install-preview";
      });
      await refreshOverview(false);
    } catch (error) {
      mutate((state) => {
        state.quick_swap.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.quick_swap.installing = false;
      });
    }
  }

  function selectInstalledProfile(profileName: string): void {
    mutate((state) => {
      state.restore.selected_profile_name = profileName;
      state.restore.preview = null;
      state.restore.report = null;
      state.restore.confirmation = "";
      state.restore.error = null;
    });
  }

  function setRestoreFromOriginals(value: boolean): void {
    mutate((state) => {
      state.restore.from_originals = value;
      state.restore.preview = null;
      state.restore.report = null;
      state.restore.confirmation = "";
    });
  }

  function setRestoreConfirmation(value: string): void {
    mutate((state) => {
      state.restore.confirmation = value;
    });
  }

  async function previewCurrentRestore(): Promise<void> {
    const profileName = snapshot.restore.selected_profile_name;
    if (!profileName) {
      return;
    }
    mutate((state) => {
      state.restore.previewing = true;
      state.restore.error = null;
    });
    try {
      const preview = await runLogged(
        "restore_preview",
        () => restorePreview(profileName, snapshot.restore.from_originals),
        (result) => `${result.status} for ${result.profile_name}`,
      );
      mutate((state) => {
        state.restore.preview = preview;
      });
    } catch (error) {
      mutate((state) => {
        state.restore.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.restore.previewing = false;
      });
    }
  }

  async function restoreCurrentProfile(): Promise<void> {
    const profileName = snapshot.restore.selected_profile_name;
    if (!profileName) {
      return;
    }
    mutate((state) => {
      state.restore.restoring = true;
      state.restore.error = null;
    });
    try {
      const report = await runLogged(
        "restore_confirmed",
        () =>
          restoreConfirmed(
            profileName,
            snapshot.restore.from_originals,
            snapshot.restore.confirmation,
          ),
        (result) => `${result.status} for ${result.profile_name}`,
      );
      mutate((state) => {
        state.restore.report = report;
      });
      await refreshOverview(false);
    } catch (error) {
      mutate((state) => {
        state.restore.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.restore.restoring = false;
      });
    }
  }

  async function refreshBackupStatus(): Promise<void> {
    mutate((state) => {
      state.backups.loading = true;
      state.backups.error = null;
    });
    try {
      const status = await runLogged(
        "backup_originals_status",
        backupOriginalsStatus,
        summaryFromBackupStatus,
      );
      mutate((state) => {
        state.backups.status = status;
      });
    } catch (error) {
      mutate((state) => {
        state.backups.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.backups.loading = false;
      });
    }
  }

  async function verifyBackups(): Promise<void> {
    mutate((state) => {
      state.backups.verifying = true;
      state.backups.error = null;
    });
    try {
      const status = await runLogged(
        "backup_originals_verify",
        backupOriginalsVerify,
        summaryFromBackupStatus,
      );
      mutate((state) => {
        state.backups.status = status;
      });
    } catch (error) {
      mutate((state) => {
        state.backups.error = asErrorMessage(error);
      });
    } finally {
      mutate((state) => {
        state.backups.verifying = false;
      });
    }
  }

  function noticesFrom(items: Array<UiNotice | string> | null | undefined): UiNotice[] {
    return (items ?? []).map((item) => {
      if (typeof item === "string") {
        return { message: item };
      }
      return item;
    });
  }

  return {
    subscribe,
    load,
    refreshOverview,
    browseGameFolder: () => browseFolderFor("game"),
    browseCodeRedFolder: () => browseFolderFor("codered"),
    setActivePage,
    setGamePathInput,
    setImportFolderInput,
    validateCurrentGamePath,
    saveCurrentGamePath,
    importCurrentCodeRedFolder,
    refreshDatabase,
    setSearchQuery,
    selectSearchHit,
    createCurrentPlan,
    buildCurrentPlan,
    previewCurrentInstall,
    setInstallConfirmation,
    setOverwriteProfileBackup,
    installCurrentPlan,
    selectInstalledProfile,
    setRestoreFromOriginals,
    setRestoreConfirmation,
    previewCurrentRestore,
    restoreCurrentProfile,
    refreshBackupStatus,
    verifyBackups,
    noticesFrom,
  };
}

export const appStore = createAppStore();