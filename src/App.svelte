<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { get } from "svelte/store";

  import NoticeGroup from "./lib/components/NoticeGroup.svelte";
  import { appStore } from "./lib/store";
  import type {
    BackupVerificationResult,
    InstallPreview,
    PageId,
    PlanBuildReport,
    SearchHit,
    SwapPlan,
    UiNotice,
  } from "./lib/types";

  type StatusTone = "ok" | "warn" | "danger" | "neutral";

  interface SafetyInfo {
    tone: StatusTone;
    label: string;
    detail: string;
  }

  const pages: Array<{ id: PageId; label: string; detail: string }> = [
    { id: "home", label: "Home", detail: "Overview / safety" },
    { id: "game-folder", label: "Game Folder", detail: "Validate sandbox path" },
    { id: "database", label: "Database", detail: "Import / index" },
    { id: "quick-swap", label: "Quick Swap", detail: "Search / plan / build" },
    { id: "install-preview", label: "Install Preview", detail: "Confirm install" },
    { id: "active-swaps", label: "Active Swaps", detail: "Restore / history" },
    { id: "backups", label: "Backups", detail: "Original backup health" },
    { id: "diagnostics", label: "Diagnostics", detail: "Local paths / counts" },
    { id: "logs", label: "Logs", detail: "Command console" },
  ];

  const safetyRules = [
    "Offline and local only. Do not use this tool online and do not bypass EAC.",
    "Sandbox-first bring-up. Validate against copied or fake CookedPCConsole roots before touching a live install.",
    "Planning, rebuild, install, and restore logic stay in Rust. The frontend only orchestrates backend contracts.",
  ];

  const smokeLogSequence = [
    "validate_game_path",
    "set_game_path",
    "import_codered",
    "refresh_db",
    "search_items",
    "create_plan",
    "build_plan",
    "install_preview",
    "install_confirmed",
    "list_installed_swaps",
    "restore_preview",
    "restore_confirmed",
    "backup_originals_status",
    "backup_originals_verify",
  ];

  onMount(() => {
    void appStore.load();
  });

  let pageBadges: Record<PageId, string | null> = {
    home: null,
    "game-folder": null,
    database: null,
    "quick-swap": null,
    "install-preview": null,
    "active-swaps": null,
    backups: null,
    diagnostics: null,
    logs: null,
  };
  let currentPage = pages[0];
  let logCopyState: "idle" | "copied" | "failed" = "idle";
  let logCopyResetHandle: number | null = null;

  onDestroy(() => {
    if (typeof window !== "undefined" && logCopyResetHandle !== null) {
      window.clearTimeout(logCopyResetHandle);
    }
  });

  $: selectedSwap =
    $appStore.restore.installed_swaps.find(
      (swap) => swap.profile_name === $appStore.restore.selected_profile_name,
    ) ?? null;
  $: currentPage = pages.find((page) => page.id === $appStore.active_page) ?? pages[0];

  $: configuredCookedPath =
    $appStore.config?.cooked_dir ?? $appStore.app_status?.configured_cooked_dir ?? null;
  $: configuredCookedSafety = describePathSafety(configuredCookedPath, $appStore.config?.app_home);
  $: validationCookedSafety = describePathSafety(
    $appStore.setup.validation?.normalized_cooked_dir,
    $appStore.config?.app_home,
  );
  $: installCookedPath =
    $appStore.quick_swap.install_preview?.configured_cooked_root ?? configuredCookedPath;
  $: installCookedSafety = describePathSafety(installCookedPath, $appStore.config?.app_home);
  $: restoreCookedPath = selectedSwap?.cooked_root ?? configuredCookedPath;
  $: restoreCookedSafety = describePathSafety(restoreCookedPath, $appStore.config?.app_home);
  $: selectionReadiness = describeSelectionPreflight(
    $appStore.quick_swap.target.selected,
    $appStore.quick_swap.source.selected,
  );
  $: backupHealth = describeBackupHealth($appStore.backups.status);
  $: profileBackupRoot = appendPath($appStore.config?.app_home, "workspace", "backups");
  $: gamePathReadyToSave =
    $appStore.setup.validation?.is_valid === true &&
    $appStore.setup.validation.input_path === $appStore.setup.game_path_input.trim();
  $: databaseReadyToRefresh =
    Boolean(configuredCookedPath) && Boolean($appStore.config?.codered_dumps_dir);
  $: installPreviewReady =
    $appStore.quick_swap.install_preview?.status === "preview_ready" &&
    ($appStore.quick_swap.install_preview?.blockers.length ?? 0) === 0;
  $: installPhraseMatches =
    $appStore.quick_swap.install_confirmation.trim() ===
    ($appStore.quick_swap.install_preview?.confirmation_phrase ?? "");
  $: restorePreviewReady =
    $appStore.restore.preview?.status === "preview_ready" &&
    ($appStore.restore.preview?.blockers.length ?? 0) === 0;
  $: restorePhraseMatches =
    $appStore.restore.confirmation.trim() === ($appStore.restore.preview?.confirmation_phrase ?? "");
  $: quickSwapNextStep = describeQuickSwapNextStep(
    $appStore.quick_swap.target.selected,
    $appStore.quick_swap.source.selected,
    $appStore.quick_swap.plan,
    $appStore.quick_swap.build_report,
    $appStore.quick_swap.install_preview,
  );
  $: pageBadges = {
    home: null,
    "game-folder": configuredCookedPath
      ? $appStore.app_status
        ? `${$appStore.app_status.local_files_count} upk`
        : "Configured"
      : "Needs setup",
    database: $appStore.app_status ? `${$appStore.app_status.product_count} items` : null,
    "quick-swap": humanizeToken($appStore.quick_swap.build_report?.status) ?? ($appStore.quick_swap.plan ? "plan ready" : null),
    "install-preview": humanizeToken($appStore.quick_swap.install_preview?.status),
    "active-swaps": $appStore.app_status ? `${$appStore.app_status.active_swap_count} active` : null,
    backups: $appStore.backups.status
      ? `${$appStore.backups.status.tracked_file_count} tracked`
      : null,
    diagnostics: $appStore.config ? "local paths" : null,
    logs: `${$appStore.logs.length}`,
  };

  function noticesFrom(items: Array<UiNotice | string> | null | undefined): UiNotice[] {
    return appStore.noticesFrom(items);
  }

  function shortPath(value: string | null | undefined): string {
    if (!value) {
      return "Not configured";
    }

    const cropped = cropPathToStableTail(value);
    if (cropped.length <= 72) {
      return cropped;
    }
    return `${cropped.slice(0, 26)}...${cropped.slice(-38)}`;
  }

  function displayPath(value: string | null | undefined): string {
    return shortPath(value);
  }

  function humanizeToken(value: string | null | undefined): string | null {
    if (!value) {
      return null;
    }
    return value.replace(/[_-]+/g, " ").replace(/\s+/g, " ").trim();
  }

  function cropPathToStableTail(value: string): string {
    const normalized = value.toLowerCase();
    const anchors = [
      "\\target\\gui_smoke\\",
      "/target/gui_smoke/",
      "\\app_home\\",
      "/app_home/",
      "\\workspace\\",
      "/workspace/",
      "\\bakkeswap\\",
      "/bakkeswap/",
    ];

    for (const anchor of anchors) {
      const index = normalized.indexOf(anchor);
      if (index > 0) {
        return `...${value.slice(index)}`;
      }
    }

    return value;
  }

  function formatDate(value: string | null | undefined): string {
    if (!value) {
      return "Not available";
    }
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return value;
    }
    return `${date.toLocaleDateString()} ${date.toLocaleTimeString()}`;
  }

  function normalizePath(value: string | null | undefined): string {
    return (value ?? "").replace(/\\/g, "/").toLowerCase();
  }

  function describePathSafety(path: string | null | undefined, appHome: string | null | undefined): SafetyInfo {
    if (!path) {
      return {
        tone: "neutral",
        label: "No path",
        detail: "No CookedPCConsole path is configured yet.",
      };
    }

    const normalized = normalizePath(path);
    const normalizedAppHome = normalizePath(appHome);
    const sandboxHints = [
      "/target/gui_smoke/",
      "/sandbox/",
      "/fake/",
      "/fixtures/",
      "/samples/",
      "/tmp/",
      "/temp/",
    ];
    const liveInstallHints = [
      "/steamapps/common/rocketleague",
      "/epic games/rocketleague",
      "/program files/rocketleague",
      "/program files (x86)/rocketleague",
    ];

    if (
      sandboxHints.some((hint) => normalized.includes(hint)) ||
      normalized.includes("/bakkeswap/target/") ||
      (normalizedAppHome.length > 0 && normalized.startsWith(normalizedAppHome))
    ) {
      return {
        tone: "ok",
        label: "Sandbox-safe",
        detail:
          "This path points at the fake or project-local sandbox. It is appropriate for sandbox validation and screenshots.",
      };
    }

    if (
      liveInstallHints.some((hint) => normalized.includes(hint)) ||
      (normalized.includes("/rocketleague/") && !normalized.includes("/target/gui_smoke/"))
    ) {
      return {
        tone: "danger",
        label: "Live-install risk",
        detail:
          "This path resembles a real Rocket League install. Keep install and restore on copied or fake roots until you intentionally leave sandbox validation.",
      };
    }

    return {
      tone: "warn",
      label: "Custom local path",
      detail:
        "This path is local but does not clearly look sandboxed. Confirm it is copied or fake before any risky action.",
    };
  }

  function describeSelectionPreflight(
    target: SearchHit | null,
    source: SearchHit | null,
  ): SafetyInfo {
    if (!target && !source) {
      return {
        tone: "warn",
        label: "Choose TARGET + SOURCE",
        detail:
          "Choose the item you already own or equip for TARGET, then the item you want to see for SOURCE.",
      };
    }
    if (!target || !source) {
      return {
        tone: "warn",
        label: "One side missing",
        detail: "Both TARGET and SOURCE must be selected before the backend can create a plan.",
      };
    }
    if (target.slot && source.slot && target.slot === source.slot) {
      return {
        tone: "ok",
        label: "Likely compatible",
        detail:
          "The selected items share the same slot label. The backend plan still remains the source of truth for compatibility.",
      };
    }
    if (target.slot && source.slot && target.slot !== source.slot) {
      return {
        tone: "danger",
        label: "Slot mismatch",
        detail:
          "The selected items do not share the same slot label. The backend plan will likely block build or install.",
      };
    }
    return {
      tone: "neutral",
      label: "Metadata incomplete",
      detail:
        "At least one selected item is missing slot metadata. Create a plan to let the backend confirm compatibility.",
    };
  }

  function describeBackupHealth(status: BackupVerificationResult | null): SafetyInfo {
    if (!status) {
      return {
        tone: "neutral",
        label: "No backup report",
        detail: "Load backup status before trusting restore or originals coverage.",
      };
    }
    if (
      status.missing_file_count > 0 ||
      status.mismatched_file_count > 0 ||
      status.blockers.length > 0
    ) {
      return {
        tone: "danger",
        label: "Backup issues",
        detail:
          "Some tracked originals are missing or mismatched. Review blockers before using restore workflows.",
      };
    }
    if (status.untracked_file_count > 0 || status.warnings.length > 0) {
      return {
        tone: "warn",
        label: "Review backups",
        detail:
          "Backups are present, but there are warnings or untracked files worth reviewing before risky actions.",
      };
    }
    return {
      tone: "ok",
      label: "Backups ready",
      detail: "Permanent originals are tracked and verified for the currently indexed files.",
    };
  }

  function describeQuickSwapNextStep(
    target: SearchHit | null,
    source: SearchHit | null,
    plan: SwapPlan | null,
    buildReport: PlanBuildReport | null,
    installPreview: InstallPreview | null,
  ): SafetyInfo {
    if (!target && !source) {
      return {
        tone: "warn",
        label: "Next > Search TARGET",
        detail: "Start with the item you already own or equip, then pick the SOURCE appearance you want to see.",
      };
    }

    if (!target) {
      return {
        tone: "warn",
        label: "Next > Pick TARGET",
        detail: "Choose the TARGET item first so the backend can preserve its on-disk identity.",
      };
    }

    if (!source) {
      return {
        tone: "warn",
        label: "Next > Pick SOURCE",
        detail: "Choose the SOURCE item you want to see locally before creating a plan.",
      };
    }

    if (!plan) {
      return {
        tone: "neutral",
        label: "Next > Create plan",
        detail: "Both sides are selected. Create a backend plan to confirm compatibility and filenames.",
      };
    }

    if ((plan.build_blockers.length ?? 0) > 0) {
      return {
        tone: "danger",
        label: "Plan blocked",
        detail: "The backend has already reported blockers. Review them below before trying to build.",
      };
    }

    if (!buildReport || buildReport.status !== "built") {
      return {
        tone: "neutral",
        label: "Next > Build",
        detail: "The plan is ready. Build the TARGET filenames before opening the install confirmation screen.",
      };
    }

    if (!installPreview) {
      return {
        tone: "ok",
        label: "Next > Preview install",
        detail: "Built outputs are ready. Open Install Preview to inspect changed files, backups, and the exact phrase.",
      };
    }

    return {
      tone: "ok",
      label: "Preview ready",
      detail: "Install Preview is prepared. Review the sandbox path, affected files, and the exact confirmation phrase.",
    };
  }

  function appendPath(base: string | null | undefined, ...segments: string[]): string | null {
    if (!base) {
      return null;
    }
    const separator = base.includes("\\") ? "\\" : "/";
    const cleanedBase = base.replace(/[\\/]+$/, "");
    const cleanedSegments = segments.map((segment) => segment.replace(/^[\\/]+|[\\/]+$/g, ""));
    return [cleanedBase, ...cleanedSegments].join(separator);
  }

  function canSelect(hit: SearchHit): boolean {
    return hit.kind === "product" && hit.swappable;
  }

  function actionText(loading: boolean, idle: string, busy: string): string {
    return loading ? busy : idle;
  }

  function inputValue(event: Event): string {
    return (event.currentTarget as HTMLInputElement).value;
  }

  function inputChecked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  function logKindLabel(kind: string): string {
    if (kind === "started") {
      return "RUN";
    }
    if (kind === "success") {
      return "OK";
    }
    if (kind === "error") {
      return "ERR";
    }
    return "LOG";
  }

  function queueLogCopyReset(): void {
    if (typeof window === "undefined") {
      return;
    }
    if (logCopyResetHandle !== null) {
      window.clearTimeout(logCopyResetHandle);
    }
    logCopyResetHandle = window.setTimeout(() => {
      logCopyState = "idle";
      logCopyResetHandle = null;
    }, 1800);
  }

  async function copyLogsToClipboard(): Promise<void> {
    const logEntries = get(appStore).logs;
    if (logEntries.length === 0 || typeof navigator === "undefined" || !navigator.clipboard) {
      logCopyState = "failed";
      queueLogCopyReset();
      return;
    }

    const text = logEntries
      .map((log) => `${formatDate(log.at)} [${log.kind.toUpperCase()}] ${log.command} :: ${log.detail}`)
      .join("\n");

    try {
      await navigator.clipboard.writeText(text);
      logCopyState = "copied";
    } catch {
      logCopyState = "failed";
    }

    queueLogCopyReset();
  }
</script>

<svelte:head>
  <title>BakkesSwap Desktop</title>
  <meta
    name="description"
    content="Local-only BakkesSwap desktop GUI over the Rust backend contracts."
  />
</svelte:head>

<div class="shell">
  <aside class="sidebar">
    <section class="panel brand">
      <div class="brand-copy">
        <p class="eyebrow">BakkesSwap</p>
        <h1>Desktop Utility</h1>
        <p class="sidebar-copy">
          Compact local control surface for TARGET, SOURCE, plan, build, install, restore, and
          backup workflows. Backend logic stays in Rust.
        </p>
      </div>
      <div class="tool-row compact-wrap">
        <span class={`status-badge ${$appStore.tauri_available ? "ok" : "danger"}`}>
          {$appStore.tauri_available ? "Tauri ready" : "Browser only"}
        </span>
        <span class="status-badge neutral">Offline only</span>
      </div>
    </section>

    <section class="panel nav-panel">
      <div class="panel-header compact-header">
        <div>
          <p class="panel-label">Navigation</p>
          <h2>Tool sections</h2>
        </div>
        <span class="status-badge neutral">{pages.length} pages</span>
      </div>

      <nav class="nav">
        {#each pages as page}
          <button
            type="button"
            class:active={page.id === $appStore.active_page}
            aria-pressed={page.id === $appStore.active_page}
            on:click={() => appStore.setActivePage(page.id)}
          >
            <div class="nav-copy">
              <span class="nav-title">{page.label}</span>
              <small>{page.detail}</small>
            </div>
            {#if pageBadges[page.id]}
              <span class="status-badge neutral nav-badge">{pageBadges[page.id]}</span>
            {/if}
          </button>
        {/each}
      </nav>
    </section>

    <section class="panel sidebar-panel">
      <div class="panel-header compact-header">
        <div>
          <p class="panel-label">Bridge</p>
          <h2>Desktop runtime</h2>
        </div>
        <span class={`status-badge ${$appStore.tauri_available ? "ok" : "danger"}`}>
          {$appStore.tauri_available ? "Bridge live" : "Bridge offline"}
        </span>
      </div>
      <p>
        {$appStore.tauri_available
          ? "Frontend actions invoke Tauri commands. Planner, builder, installer, restore, and backup behavior remain backend-owned."
          : "Run with npm run tauri:dev to enable the Rust backend and native folder picker."}
      </p>
    </section>

    <section class="panel sidebar-panel">
      <div class="panel-header compact-header">
        <div>
          <p class="panel-label">Current path</p>
          <h2>CookedPCConsole</h2>
        </div>
        <span class={`status-badge ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
      </div>
      <p class="path-text" title={displayPath(configuredCookedPath)}>{displayPath(configuredCookedPath)}</p>
      <p class="context-note">{configuredCookedSafety.detail}</p>
    </section>

    <section class="panel sidebar-panel">
      <div class="panel-header compact-header">
        <div>
          <p class="panel-label">Guardrails</p>
          <h2>Non-negotiable</h2>
        </div>
        <span class="status-badge warning">Utility only</span>
      </div>
      <ul class="rule-list compact-list">
        {#each safetyRules as rule}
          <li>{rule}</li>
        {/each}
      </ul>
    </section>
  </aside>

  <main class="content">
    <header class="panel topbar">
      <div class="topbar-main">
        <div>
          <p class="panel-label">Desktop tool</p>
          <h2>{currentPage.label}</h2>
          <p>{currentPage.detail}</p>
        </div>
        <div class="tool-row compact-wrap topbar-badges">
          <span class={`status-badge ${$appStore.tauri_available ? "ok" : "danger"}`}>
            {$appStore.tauri_available ? "Tauri ready" : "Browser only"}
          </span>
          <span class={`status-badge ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
          <span class="status-badge neutral">{$appStore.app_status?.active_swap_count ?? 0} active</span>
          <span class="status-badge neutral">{$appStore.backups.status?.tracked_file_count ?? 0} backups</span>
        </div>
      </div>
      <div class="topbar-strip">
        <div class="topbar-path">
          <span class="topbar-field-label">CookedPCConsole</span>
          <span class="path-text" title={displayPath(configuredCookedPath)}>{displayPath(configuredCookedPath)}</span>
        </div>
        <div class="tool-row compact-wrap topbar-badges minor">
          <span class="status-badge neutral">{$appStore.app_status?.local_files_count ?? 0} upk</span>
          <span class="status-badge neutral">{$appStore.app_status?.product_count ?? 0} items</span>
          <span class="status-badge neutral">{$appStore.logs.length} logs</span>
        </div>
      </div>
    </header>

    {#if $appStore.runtime_error}
      <section class="banner danger">
        <strong>Runtime warning</strong>
        <p>{$appStore.runtime_error}</p>
      </section>
    {/if}

    {#if $appStore.bootstrap_loading}
      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="panel-label">Loading</p>
            <h3>Hydrating desktop state</h3>
          </div>
          <span class="status-chip warn">Working</span>
        </div>
        <p>Reading status, config, installed swaps, and permanent-original backup state.</p>
      </section>
    {:else if $appStore.active_page === "home"}
      <section class="stats-grid">
        <article class="panel metric-card">
          <p class="panel-label">Cooked path</p>
          <strong>{shortPath(configuredCookedPath)}</strong>
          <p>{configuredCookedSafety.label}</p>
        </article>
        <article class="panel metric-card">
          <p class="panel-label">Database</p>
          <strong>{$appStore.app_status?.product_count ?? 0} products</strong>
          <p>{$appStore.app_status?.title_count ?? 0} titles</p>
        </article>
        <article class="panel metric-card">
          <p class="panel-label">Swaps</p>
          <strong>{$appStore.app_status?.active_swap_count ?? 0} active</strong>
          <p>{$appStore.restore.installed_swaps.length} install records tracked</p>
        </article>
        <article class="panel metric-card">
          <p class="panel-label">Backups</p>
          <strong>{$appStore.backups.status?.tracked_file_count ?? 0} tracked</strong>
          <p>{backupHealth.label}</p>
        </article>
      </section>

      <section class="content-grid two-up">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Workspace</p>
              <h3>Current configuration</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(true)}>
              Refresh
            </button>
          </div>
          <dl class="detail-list">
            <div><dt>Game path input</dt><dd>{shortPath($appStore.config?.game_path_input)}</dd></div>
            <div><dt>CookedPCConsole</dt><dd>{shortPath(configuredCookedPath)}</dd></div>
            <div><dt>CodeRed dumps</dt><dd>{shortPath($appStore.config?.codered_dumps_dir)}</dd></div>
            <div><dt>App home</dt><dd>{shortPath($appStore.config?.app_home)}</dd></div>
            <div><dt>Database path</dt><dd>{shortPath($appStore.config?.database_path)}</dd></div>
          </dl>
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Safety posture</p>
              <h3>{configuredCookedSafety.label}</h3>
            </div>
            <span class={`status-chip ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
          </div>
          <p>{configuredCookedSafety.detail}</p>
          <NoticeGroup title="Configured path warnings" tone="warning" items={noticesFrom($appStore.config?.validation?.warnings)} />
          <NoticeGroup title="Configured path blockers" tone="danger" items={noticesFrom($appStore.config?.validation?.errors)} />
        </article>
      </section>

      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="panel-label">Recent activity</p>
            <h3>Command log snapshot</h3>
          </div>
          <button type="button" class="action-button subtle" on:click={() => appStore.setActivePage("logs")}>Open console</button>
        </div>
        {#if $appStore.logs.length === 0}
          <p class="empty-state">No backend activity yet. Validate a path or open Quick Swap to start the command log.</p>
        {:else}
          <div class="log-list compact">
            {#each $appStore.logs.slice(0, 6) as log}
              <article class={`log-entry ${log.kind}`}>
                <div>
                  <span class={`log-kind ${log.kind}`}>[{logKindLabel(log.kind)}]</span>
                  <strong>{log.command}</strong>
                  <p>{log.detail}</p>
                </div>
                <span>{formatDate(log.at)}</span>
              </article>
            {/each}
          </div>
        {/if}
      </section>
    {:else if $appStore.active_page === "game-folder"}
      <section class="content-grid two-up">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Setup</p>
              <h3>Select Rocket League root, TAGame, or CookedPCConsole</h3>
            </div>
            <span class={`status-chip ${gamePathReadyToSave ? "ok" : "warn"}`}>
              {gamePathReadyToSave ? "Ready to save" : "Needs validation"}
            </span>
          </div>
          <label class="field">
            <span>Game folder</span>
            <input
              type="text"
              value={$appStore.setup.game_path_input}
              placeholder="Choose Rocket League root, TAGame, or CookedPCConsole"
              on:input={(event) => appStore.setGamePathInput(inputValue(event))}
            />
          </label>
          <p class="field-note">
            Phase 5C click-through validation should still point at a fake or copied root such as
            <strong>target\gui_smoke\RocketLeague</strong>.
          </p>
          <div class="action-row">
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseGameFolder()}>Browse...</button>
            <button type="button" class="action-button" disabled={$appStore.setup.validating || !$appStore.setup.game_path_input.trim()} on:click={() => void appStore.validateCurrentGamePath()}>
              {actionText($appStore.setup.validating, "Validate", "Validating")}
            </button>
            <button type="button" class="action-button accent" disabled={!gamePathReadyToSave || $appStore.setup.saving} on:click={() => void appStore.saveCurrentGamePath()}>
              {actionText($appStore.setup.saving, "Save path", "Saving")}
            </button>
          </div>
          {#if $appStore.setup.error}
            <section class="inline-alert danger">{$appStore.setup.error}</section>
          {/if}
          <NoticeGroup title="Validation warnings" tone="warning" items={noticesFrom($appStore.setup.validation?.warnings)} />
          <NoticeGroup title="Validation blockers" tone="danger" items={noticesFrom($appStore.setup.validation?.errors)} />
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Validation details</p>
              <h3>{validationCookedSafety.label}</h3>
            </div>
            <span class={`status-chip ${validationCookedSafety.tone}`}>{validationCookedSafety.label}</span>
          </div>
          <p>{validationCookedSafety.detail}</p>
          <dl class="detail-list">
            <div><dt>Input kind</dt><dd>{$appStore.setup.validation?.input_kind ?? "Unknown"}</dd></div>
            <div><dt>Normalized cooked dir</dt><dd>{shortPath($appStore.setup.validation?.normalized_cooked_dir)}</dd></div>
            <div><dt>Input exists</dt><dd>{$appStore.setup.validation?.input_exists ? "Yes" : "No"}</dd></div>
            <div><dt>Cooked exists</dt><dd>{$appStore.setup.validation?.cooked_exists ? "Yes" : "No"}</dd></div>
            <div><dt>Visible .upk files</dt><dd>{$appStore.setup.validation?.upk_count ?? 0}</dd></div>
          </dl>
          {#if ($appStore.setup.validation?.sample_upks.length ?? 0) > 0}
            <div class="sample-list">
              {#each $appStore.setup.validation?.sample_upks ?? [] as sample}
                <span>{sample}</span>
              {/each}
            </div>
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "database"}
      <section class="content-grid two-up">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">CodeRed import</p>
              <h3>Import fake or copied dump metadata</h3>
            </div>
            <span class="status-chip neutral">Backend-owned</span>
          </div>
          <label class="field">
            <span>Dump folder</span>
            <input
              type="text"
              value={$appStore.database.import_folder_input}
              placeholder="Select the folder containing ProductDump.json"
              on:input={(event) => appStore.setImportFolderInput(inputValue(event))}
            />
          </label>
          <p class="field-note">
            The controlled sandbox helper copies a safe fixture to <strong>target\gui_smoke\codered_dumps</strong>.
          </p>
          <div class="action-row">
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseCodeRedFolder()}>Browse...</button>
            <button type="button" class="action-button accent" disabled={!$appStore.database.import_folder_input.trim() || $appStore.database.importing} on:click={() => void appStore.importCurrentCodeRedFolder()}>
              {actionText($appStore.database.importing, "Import metadata", "Importing")}
            </button>
          </div>
          {#if $appStore.database.last_import_summary}
            <dl class="detail-list">
              <div><dt>Products</dt><dd>{$appStore.database.last_import_summary.imported_products}</dd></div>
              <div><dt>Slots</dt><dd>{$appStore.database.last_import_summary.imported_slots}</dd></div>
              <div><dt>Paints</dt><dd>{$appStore.database.last_import_summary.imported_paints}</dd></div>
              <div><dt>Titles</dt><dd>{$appStore.database.last_import_summary.imported_titles}</dd></div>
            </dl>
          {/if}
          {#if $appStore.database.error}
            <section class="inline-alert danger">{$appStore.database.error}</section>
          {/if}
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Refresh</p>
              <h3>Re-import dumps and refresh the local .upk index</h3>
            </div>
            <button type="button" class="action-button" disabled={!databaseReadyToRefresh || $appStore.database.refreshing} on:click={() => void appStore.refreshDatabase()}>
              {actionText($appStore.database.refreshing, "Refresh database", "Refreshing")}
            </button>
          </div>
          <p>
            Refresh remains disabled until both a configured CookedPCConsole and a saved CodeRed dump
            folder are available.
          </p>
          <dl class="detail-list">
            <div><dt>Configured CookedPCConsole</dt><dd>{shortPath(configuredCookedPath)}</dd></div>
            <div><dt>Saved dump folder</dt><dd>{shortPath($appStore.config?.codered_dumps_dir)}</dd></div>
            <div><dt>Products</dt><dd>{$appStore.app_status?.product_count ?? 0}</dd></div>
            <div><dt>Titles</dt><dd>{$appStore.app_status?.title_count ?? 0}</dd></div>
            <div><dt>Local .upk indexed</dt><dd>{$appStore.app_status?.local_files_count ?? 0}</dd></div>
          </dl>
          {#if $appStore.database.last_refresh_result}
            <NoticeGroup title="Refresh warnings" tone="warning" items={noticesFrom($appStore.database.last_refresh_result.warnings)} />
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "quick-swap"}
      <section class="content-grid two-up quick-swap-grid">
        <article class="panel search-panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">TARGET</p>
              <h3>Item you own or equip</h3>
            </div>
            <span class="status-badge neutral">Products only</span>
          </div>
          <p class="helper-copy">
            Choose the product you already own or equip in Rocket League. This identity stays on disk.
          </p>
          <label class="field">
            <span>Search TARGET</span>
            <input type="text" value={$appStore.quick_swap.target.query} placeholder="Search product id, name, slot, or package" on:input={(event) => appStore.setSearchQuery("target", inputValue(event))} />
          </label>
          <div class="result-list">
            {#if $appStore.quick_swap.target.loading}
              <p class="empty-state">Searching TARGET catalog...</p>
            {:else if $appStore.quick_swap.target.results.length === 0}
              <p class="empty-state">Next: enter a TARGET query to load preserved-item candidates.</p>
            {:else}
              {#each $appStore.quick_swap.target.results as hit}
                <button type="button" class:selected={$appStore.quick_swap.target.selected?.id === hit.id} class:disabled={!canSelect(hit)} disabled={!canSelect(hit)} on:click={() => appStore.selectSearchHit("target", hit)}>
                  <div>
                    <strong>{hit.name}</strong>
                    <p>#{hit.id}</p>
                  </div>
                  <div>
                    <span>{hit.slot ?? "Unknown slot"}</span>
                    <small>{hit.quality ?? hit.note ?? "No quality metadata"}</small>
                  </div>
                </button>
              {/each}
            {/if}
          </div>
          {#if $appStore.quick_swap.target.error}
            <section class="inline-alert danger">{$appStore.quick_swap.target.error}</section>
          {/if}

          <section class="selection-card">
            {#if $appStore.quick_swap.target.selected}
              <div class="panel-header compact-header">
                <div>
                  <p class="panel-label">Selected TARGET</p>
                  <h4>{$appStore.quick_swap.target.selected.name}</h4>
                </div>
                <span class="status-badge ok">#{$appStore.quick_swap.target.selected.id}</span>
              </div>
              <dl class="detail-list compact-table key-value-table">
                <div><dt>Product ID</dt><dd>{$appStore.quick_swap.target.selected.id}</dd></div>
                <div><dt>Slot</dt><dd>{$appStore.quick_swap.target.selected.slot ?? "Unknown"}</dd></div>
                <div><dt>Quality</dt><dd>{$appStore.quick_swap.target.selected.quality ?? "Unknown"}</dd></div>
                <div><dt>Visual package</dt><dd class="path-text" title={displayPath($appStore.quick_swap.target.selected.product_asset_package)}>{displayPath($appStore.quick_swap.target.selected.product_asset_package)}</dd></div>
                <div><dt>Thumbnail package</dt><dd class="path-text" title={displayPath($appStore.quick_swap.target.selected.product_thumbnail_package)}>{displayPath($appStore.quick_swap.target.selected.product_thumbnail_package)}</dd></div>
              </dl>
            {:else}
              <div class="selection-card-empty">
                <p class="panel-label">Selected TARGET</p>
                <p class="empty-state">Pick the owned or equipped item whose on-disk identity should stay intact.</p>
              </div>
            {/if}
          </section>
        </article>

        <article class="panel search-panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">SOURCE</p>
              <h3>Item you want to see</h3>
            </div>
            <span class="status-badge neutral">Products only</span>
          </div>
          <p class="helper-copy">
            Choose the local appearance you want to see. SOURCE content is rebuilt into TARGET filenames.
          </p>
          <label class="field">
            <span>Search SOURCE</span>
            <input type="text" value={$appStore.quick_swap.source.query} placeholder="Search source product id, name, slot, or package" on:input={(event) => appStore.setSearchQuery("source", inputValue(event))} />
          </label>
          <div class="result-list">
            {#if $appStore.quick_swap.source.loading}
              <p class="empty-state">Searching SOURCE catalog...</p>
            {:else if $appStore.quick_swap.source.results.length === 0}
              <p class="empty-state">Next: enter a SOURCE query to load appearance candidates.</p>
            {:else}
              {#each $appStore.quick_swap.source.results as hit}
                <button type="button" class:selected={$appStore.quick_swap.source.selected?.id === hit.id} class:disabled={!canSelect(hit)} disabled={!canSelect(hit)} on:click={() => appStore.selectSearchHit("source", hit)}>
                  <div>
                    <strong>{hit.name}</strong>
                    <p>#{hit.id}</p>
                  </div>
                  <div>
                    <span>{hit.slot ?? "Unknown slot"}</span>
                    <small>{hit.quality ?? hit.note ?? "No quality metadata"}</small>
                  </div>
                </button>
              {/each}
            {/if}
          </div>
          {#if $appStore.quick_swap.source.error}
            <section class="inline-alert danger">{$appStore.quick_swap.source.error}</section>
          {/if}

          <section class="selection-card">
            {#if $appStore.quick_swap.source.selected}
              <div class="panel-header compact-header">
                <div>
                  <p class="panel-label">Selected SOURCE</p>
                  <h4>{$appStore.quick_swap.source.selected.name}</h4>
                </div>
                <span class="status-badge ok">#{$appStore.quick_swap.source.selected.id}</span>
              </div>
              <dl class="detail-list compact-table key-value-table">
                <div><dt>Product ID</dt><dd>{$appStore.quick_swap.source.selected.id}</dd></div>
                <div><dt>Slot</dt><dd>{$appStore.quick_swap.source.selected.slot ?? "Unknown"}</dd></div>
                <div><dt>Quality</dt><dd>{$appStore.quick_swap.source.selected.quality ?? "Unknown"}</dd></div>
                <div><dt>Visual package</dt><dd class="path-text" title={displayPath($appStore.quick_swap.source.selected.product_asset_package)}>{displayPath($appStore.quick_swap.source.selected.product_asset_package)}</dd></div>
                <div><dt>Thumbnail package</dt><dd class="path-text" title={displayPath($appStore.quick_swap.source.selected.product_thumbnail_package)}>{displayPath($appStore.quick_swap.source.selected.product_thumbnail_package)}</dd></div>
              </dl>
            {:else}
              <div class="selection-card-empty">
                <p class="panel-label">Selected SOURCE</p>
                <p class="empty-state">Pick the visual item you want the rebuilt TARGET files to show locally.</p>
              </div>
            {/if}
          </section>
        </article>
      </section>

      <section class="panel">
        <div class="panel-header">
          <div>
            <p class="panel-label">Compatibility</p>
            <h3>{$appStore.quick_swap.plan?.profile_name ?? selectionReadiness.label}</h3>
          </div>
          <div class="tool-row compact-wrap">
            <span class={`status-badge ${selectionReadiness.tone}`}>{selectionReadiness.label}</span>
            {#if $appStore.quick_swap.plan}
              <span class={`status-badge ${$appStore.quick_swap.plan.compatibility.same_slot ? "ok" : "danger"}`}>
                {$appStore.quick_swap.plan.compatibility.same_slot ? "Same slot confirmed" : "Slot mismatch"}
              </span>
            {/if}
          </div>
        </div>

        <p class="helper-copy">{selectionReadiness.detail}</p>

        <article class={`notice-strip ${quickSwapNextStep.tone}`}>
          <strong>{quickSwapNextStep.label}</strong>
          <p>{quickSwapNextStep.detail}</p>
        </article>

        <dl class="detail-list compact-table key-value-table compatibility-table">
          <div><dt>Configured CookedPCConsole</dt><dd class="path-text" title={displayPath(configuredCookedPath)}>{displayPath(configuredCookedPath)}</dd></div>
          <div><dt>Path posture</dt><dd>{configuredCookedSafety.label}</dd></div>
          <div><dt>Plan profile</dt><dd>{$appStore.quick_swap.plan?.profile_name ?? "Not created"}</dd></div>
          <div><dt>Build status</dt><dd>{humanizeToken($appStore.quick_swap.build_report?.status) ?? "Not built"}</dd></div>
          <div><dt>TARGET</dt><dd>{$appStore.quick_swap.target.selected?.name ?? "Not selected"}</dd></div>
          <div><dt>SOURCE</dt><dd>{$appStore.quick_swap.source.selected?.name ?? "Not selected"}</dd></div>
          <div><dt>Plan file</dt><dd class="path-text" title={displayPath($appStore.quick_swap.plan?.plan_path)}>{displayPath($appStore.quick_swap.plan?.plan_path)}</dd></div>
          <div><dt>Install preview</dt><dd>{humanizeToken($appStore.quick_swap.install_preview?.status) ?? "Not prepared"}</dd></div>
        </dl>

        <div class="tool-row action-bar">
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.target.selected || !$appStore.quick_swap.source.selected || $appStore.quick_swap.creating_plan} on:click={() => void appStore.createCurrentPlan()}>
              {actionText($appStore.quick_swap.creating_plan, "Create plan", "Planning")}
            </button>
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.plan || ($appStore.quick_swap.plan?.build_blockers.length ?? 0) > 0 || $appStore.quick_swap.building} on:click={() => void appStore.buildCurrentPlan()}>
              {actionText($appStore.quick_swap.building, "Build", "Building")}
            </button>
            <button type="button" class="action-button accent" disabled={!$appStore.quick_swap.build_report || $appStore.quick_swap.build_report.status !== "built" || $appStore.quick_swap.previewing_install} on:click={() => void appStore.previewCurrentInstall()}>
              {actionText($appStore.quick_swap.previewing_install, "Preview install", "Preparing preview")}
            </button>
            <button type="button" class="action-button subtle" disabled={!$appStore.quick_swap.plan} on:click={() => appStore.setActivePage("install-preview")}>
              Open install
            </button>
        </div>

        {#if $appStore.quick_swap.plan}
          <NoticeGroup title="Plan warnings" tone="warning" items={noticesFrom($appStore.quick_swap.plan.warnings)} />
          <NoticeGroup title="Plan blockers" tone="danger" items={noticesFrom($appStore.quick_swap.plan.build_blockers)} />
        {/if}
        {#if $appStore.quick_swap.build_report}
          <NoticeGroup title="Build warnings" tone="warning" items={noticesFrom($appStore.quick_swap.build_report.warnings)} />
          <NoticeGroup title="Build blockers" tone="danger" items={noticesFrom($appStore.quick_swap.build_report.blockers)} />
        {/if}
        {#if $appStore.quick_swap.error}
          <section class="inline-alert danger">{$appStore.quick_swap.error}</section>
        {/if}
      </section>
    {:else if $appStore.active_page === "install-preview"}
      <section class="content-grid wide-right install-layout">
        <article class="panel context-card install-gate-panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Install confirmation</p>
              <h3>{$appStore.quick_swap.install_preview?.profile_name ?? $appStore.quick_swap.plan?.profile_name ?? "No plan selected"}</h3>
            </div>
            <span class={`status-badge ${installPreviewReady ? "ok" : "warning"}`}>
              {humanizeToken($appStore.quick_swap.install_preview?.status) ?? "waiting"}
            </span>
          </div>

          {#if !$appStore.quick_swap.plan}
            <p class="empty-state">Next: create a plan, build it, then return here to review the install confirmation screen.</p>
          {:else}
            <dl class="detail-list compact-table key-value-table">
              <div><dt>Configured CookedPCConsole</dt><dd class="path-text" title={displayPath(installCookedPath)}>{displayPath(installCookedPath)}</dd></div>
              <div><dt>Path posture</dt><dd>{installCookedSafety.label}</dd></div>
              <div><dt>Plan file</dt><dd class="path-text" title={displayPath($appStore.quick_swap.plan.plan_path)}>{displayPath($appStore.quick_swap.plan.plan_path)}</dd></div>
              <div><dt>Build status</dt><dd>{humanizeToken($appStore.quick_swap.build_report?.status) ?? "Not built"}</dd></div>
              <div><dt>Build root</dt><dd class="path-text" title={displayPath($appStore.quick_swap.install_preview?.build_root)}>{displayPath($appStore.quick_swap.install_preview?.build_root)}</dd></div>
              <div><dt>Workspace root</dt><dd class="path-text" title={displayPath($appStore.quick_swap.install_preview?.workspace_root)}>{displayPath($appStore.quick_swap.install_preview?.workspace_root)}</dd></div>
            </dl>

            <article class={`notice-strip ${installCookedSafety.tone}`}>
              <strong>{installCookedSafety.label}</strong>
              <p>{installCookedSafety.detail}</p>
            </article>

            <article class="notice-strip neutral">
              <strong>Step 1 review files. Step 2 type phrase. Step 3 confirm install.</strong>
              <p>The backend preview remains the source of truth. This page only stages the final confirmation.</p>
            </article>

            <div class="tool-row compact-wrap">
              <span class={`status-badge ${installPhraseMatches ? "ok" : "warning"}`}>
                {installPhraseMatches ? "Phrase matched" : "Type exact phrase"}
              </span>
              <span class="status-badge neutral">Preview required</span>
            </div>

            <div class="tool-row">
              <button type="button" class="action-button" disabled={!$appStore.quick_swap.build_report || $appStore.quick_swap.build_report.status !== "built" || $appStore.quick_swap.previewing_install} on:click={() => void appStore.previewCurrentInstall()}>
                {actionText($appStore.quick_swap.previewing_install, "Refresh preview", "Preparing preview")}
              </button>
            </div>
          {/if}
        </article>

        <article class="panel install-backup-panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Backup paths</p>
              <h3>Install safety inputs</h3>
            </div>
            <span class="status-badge neutral">Review backups</span>
          </div>

          {#if !$appStore.quick_swap.install_preview}
            <p class="empty-state">Generate a preview first so this page can show backup roots, manifest targets, and restore helpers.</p>
          {:else}
            <dl class="detail-list compact-table key-value-table">
              <div><dt>Original backup manifest</dt><dd class="path-text" title={displayPath($appStore.quick_swap.install_preview.original_backup_manifest_path)}>{displayPath($appStore.quick_swap.install_preview.original_backup_manifest_path)}</dd></div>
              <div><dt>Restore command</dt><dd class="path-text" title={displayPath($appStore.quick_swap.install_preview.restore_command)}>{displayPath($appStore.quick_swap.install_preview.restore_command)}</dd></div>
            </dl>

            <p class="subpanel-label">Profile backups</p>
            <div class="compact-table file-table">
              <div class="table-row table-head"><span>Kind</span><span>Operation</span><span>Path</span><span>Status</span></div>
              {#each $appStore.quick_swap.install_preview.profile_backups as backup}
                <div class="table-row">
                  <span>{backup.backup_kind}</span>
                  <span>{backup.operation_kind}</span>
                  <span class="path-text" title={displayPath(backup.backup_path)}>{displayPath(backup.backup_path)}</span>
                  <span>{backup.status}</span>
                </div>
              {/each}
            </div>

            <p class="subpanel-label">Permanent originals</p>
            <div class="compact-table file-table">
              <div class="table-row table-head"><span>Kind</span><span>Operation</span><span>Path</span><span>Status</span></div>
              {#each $appStore.quick_swap.install_preview.permanent_original_backups as backup}
                <div class="table-row">
                  <span>{backup.backup_kind}</span>
                  <span>{backup.operation_kind}</span>
                  <span class="path-text" title={displayPath(backup.backup_path)}>{displayPath(backup.backup_path)}</span>
                  <span>{backup.status}</span>
                </div>
              {/each}
            </div>
          {/if}
        </article>
      </section>

      <section class="panel install-files-panel">
        <div class="panel-header">
          <div>
            <p class="panel-label">Affected files</p>
            <h3>Final review</h3>
          </div>
          <span class={`status-badge ${installPhraseMatches ? "ok" : "warning"}`}>{installPhraseMatches ? "Ready for confirm" : "Awaiting exact phrase"}</span>
        </div>

        {#if !$appStore.quick_swap.install_preview}
          <p class="empty-state">No preview loaded yet. Open Quick Swap, build the plan, then generate a preview before confirming install here.</p>
        {:else}
          <div class="compact-table file-table">
            <div class="table-row table-head"><span>Kind</span><span>Filename</span><span>Destination</span><span>Result</span></div>
            {#each $appStore.quick_swap.install_preview.files as file}
              <div class="table-row">
                <span>{file.kind}</span>
                <span>{file.target_filename}</span>
                <span class="path-text" title={displayPath(file.target_path)}>{displayPath(file.target_path)}</span>
                <span>{file.would_overwrite ? "Would overwrite" : "Fresh copy"}</span>
              </div>
            {/each}
          </div>

          <NoticeGroup title="Preview warnings" tone="warning" items={noticesFrom($appStore.quick_swap.install_preview.warnings)} />
          <NoticeGroup title="Preview blockers" tone="danger" items={noticesFrom($appStore.quick_swap.install_preview.blockers)} />

          <article class="confirm-card tool-window-card">
            <div class="panel-header compact-header">
              <div>
                <p class="panel-label">Typed confirmation</p>
                <h4>{$appStore.quick_swap.install_preview.confirmation_phrase}</h4>
              </div>
              <span class={`status-badge ${installPhraseMatches ? "ok" : "warning"}`}>{installPhraseMatches ? "Phrase matched" : "Type exact phrase"}</span>
            </div>
            <div class="confirmation-phrase">{$appStore.quick_swap.install_preview.confirmation_phrase}</div>
            <label class="field">
              <span>Confirmation phrase</span>
              <input type="text" value={$appStore.quick_swap.install_confirmation} placeholder="Type the exact confirmation phrase" on:input={(event) => appStore.setInstallConfirmation(inputValue(event))} />
            </label>
            <label class="checkbox-field">
              <input type="checkbox" checked={$appStore.quick_swap.overwrite_profile_backup} on:change={(event) => appStore.setOverwriteProfileBackup(inputChecked(event))} />
              <span>Allow overwriting the profile backup if one already exists</span>
            </label>
            <button type="button" class="action-button accent" disabled={!installPreviewReady || !installPhraseMatches || $appStore.quick_swap.installing} on:click={() => void appStore.installCurrentPlan()}>
              {actionText($appStore.quick_swap.installing, "Install confirmed", "Installing")}
            </button>
          </article>

          {#if $appStore.quick_swap.install_report}
            <section class={`inline-alert ${$appStore.quick_swap.install_report.installed ? "success" : "danger"}`}>
              {$appStore.quick_swap.install_report.status} at {formatDate($appStore.quick_swap.install_report.installed_at)}
            </section>
            {#if !$appStore.quick_swap.install_report.installed}
              <p class="context-note">
                Install did not complete. Review the execution blockers below. If this is a repeat
                install into the same sandbox profile, enable profile backup overwrite and retry.
              </p>
            {/if}
            <NoticeGroup title="Execution warnings" tone="warning" items={noticesFrom($appStore.quick_swap.install_report.warnings)} />
            <NoticeGroup title="Execution blockers" tone="danger" items={noticesFrom($appStore.quick_swap.install_report.blockers)} />
          {/if}
        {/if}
      </section>
    {:else if $appStore.active_page === "active-swaps"}
      <section class="content-grid active-swaps-layout">
        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Installed profiles</p>
              <h3>Restore targets</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Refresh list</button>
          </div>
          {#if $appStore.restore.installed_swaps.length === 0}
            <p class="empty-state">No installed swap records yet. Complete a sandbox install once to populate restore history here.</p>
          {:else}
            <div class="compact-table swap-table">
              <div class="table-row table-head"><span>Profile</span><span>Target</span><span>Source</span><span>Status</span><span>Action</span></div>
              {#each $appStore.restore.installed_swaps as swap}
                <div class={`table-row swap-row ${$appStore.restore.selected_profile_name === swap.profile_name ? "selected" : ""}`}>
                  <span>{swap.profile_name}</span>
                  <span>{swap.target_name ?? "Unknown target"}</span>
                  <span>{swap.source_name ?? "Unknown source"}</span>
                  <span>{swap.active ? "Active" : "Inactive"}</span>
                  <button type="button" class="action-button subtle row-action" on:click={() => appStore.selectInstalledProfile(swap.profile_name)}>Open</button>
                </div>
              {/each}
            </div>
          {/if}
        </article>

        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Restore</p>
              <h3>{selectedSwap?.profile_name ?? "Choose a profile"}</h3>
            </div>
            {#if selectedSwap}
              <span class={`status-badge ${selectedSwap.active ? "ok" : "warning"}`}>{selectedSwap.active ? "Active install" : "Historical record"}</span>
            {/if}
          </div>
          {#if !selectedSwap}
            <p class="empty-state">Select a row above to load restore preview tools and the exact confirmation phrase.</p>
          {:else}
            <dl class="detail-list compact-table key-value-table">
              <div><dt>TARGET</dt><dd>{selectedSwap.target_name ?? "Unknown"}</dd></div>
              <div><dt>SOURCE</dt><dd>{selectedSwap.source_name ?? "Unknown"}</dd></div>
              <div><dt>Selected install root</dt><dd class="path-text" title={displayPath(selectedSwap.cooked_root)}>{displayPath(selectedSwap.cooked_root)}</dd></div>
              <div><dt>Configured CookedPCConsole</dt><dd class="path-text" title={displayPath(configuredCookedPath)}>{displayPath(configuredCookedPath)}</dd></div>
              <div><dt>Configured path posture</dt><dd>{restoreCookedSafety.label}</dd></div>
              <div><dt>Plan status</dt><dd>{humanizeToken(selectedSwap.plan_status) ?? "Unknown"}</dd></div>
            </dl>
            <article class={`notice-strip ${restoreCookedSafety.tone}`}>
              <strong>{restoreCookedSafety.label}</strong>
              <p>{restoreCookedSafety.detail}</p>
            </article>
            <article class="notice-strip neutral">
              <strong>Step 1 preview restore. Step 2 type phrase. Step 3 confirm restore.</strong>
              <p>Use profile backup by default. Emergency originals mode should stay exceptional.</p>
            </article>
            <label class="checkbox-field danger-toggle">
              <input type="checkbox" checked={$appStore.restore.from_originals} on:change={(event) => appStore.setRestoreFromOriginals(inputChecked(event))} />
              <span>Emergency mode: restore from permanent originals instead of the profile backup</span>
            </label>
            <div class="action-row">
              <button type="button" class="action-button" disabled={$appStore.restore.previewing} on:click={() => void appStore.previewCurrentRestore()}>
                {actionText($appStore.restore.previewing, "Load restore preview", "Preparing preview")}
              </button>
            </div>
            {#if $appStore.restore.preview}
              <div class="compact-table file-table">
                <div class="table-row table-head"><span>Kind</span><span>Backup</span><span>Destination</span><span>Status</span></div>
                {#each $appStore.restore.preview.files as file}
                  <div class="table-row">
                    <span>{file.kind}</span>
                    <span>{file.backup_kind}</span>
                    <span class="path-text" title={displayPath(file.destination_path)}>{displayPath(file.destination_path)}</span>
                    <span>{file.destination_exists ? "Destination exists" : "Destination missing"}</span>
                  </div>
                {/each}
              </div>
              <NoticeGroup title="Restore warnings" tone="warning" items={noticesFrom($appStore.restore.preview.warnings)} />
              <NoticeGroup title="Restore blockers" tone="danger" items={noticesFrom($appStore.restore.preview.blockers)} />
              <article class="confirm-card tool-window-card">
                <div class="panel-header compact-header">
                  <div>
                    <p class="panel-label">Typed confirmation</p>
                    <h4>{$appStore.restore.preview.confirmation_phrase}</h4>
                  </div>
                  <span class={`status-badge ${restorePhraseMatches ? "ok" : "warning"}`}>{restorePhraseMatches ? "Phrase matched" : "Type exact phrase"}</span>
                </div>
                <div class="confirmation-phrase">{$appStore.restore.preview.confirmation_phrase}</div>
                <label class="field">
                  <span>Confirmation phrase</span>
                  <input type="text" value={$appStore.restore.confirmation} placeholder="Type the restore confirmation phrase" on:input={(event) => appStore.setRestoreConfirmation(inputValue(event))} />
                </label>
                <button type="button" class="action-button accent" disabled={!restorePreviewReady || !restorePhraseMatches || $appStore.restore.restoring} on:click={() => void appStore.restoreCurrentProfile()}>
                  {actionText($appStore.restore.restoring, "Confirm restore", "Restoring")}
                </button>
              </article>
            {/if}
            {#if $appStore.restore.report}
              <section class={`inline-alert ${$appStore.restore.report.restored ? "success" : "danger"}`}>
                {$appStore.restore.report.status} at {formatDate($appStore.restore.report.restored_at)}
              </section>
              <NoticeGroup title="Restore execution warnings" tone="warning" items={noticesFrom($appStore.restore.report.warnings)} />
              <NoticeGroup title="Restore execution blockers" tone="danger" items={noticesFrom($appStore.restore.report.blockers)} />
            {/if}
            {#if $appStore.restore.error}
              <section class="inline-alert danger">{$appStore.restore.error}</section>
            {/if}
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "backups"}
      <section class="content-grid two-up">
        <article class="panel context-card">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Backup posture</p>
              <h3>{backupHealth.label}</h3>
            </div>
            <span class={`status-chip ${backupHealth.tone}`}>{backupHealth.label}</span>
          </div>
          <p>{backupHealth.detail}</p>
          <dl class="detail-list compact">
            <div><dt>Configured CookedPCConsole</dt><dd>{shortPath(configuredCookedPath)}</dd></div>
            <div><dt>Profile backup root</dt><dd>{shortPath(profileBackupRoot)}</dd></div>
            <div><dt>Original backup root</dt><dd>{shortPath($appStore.backups.status?.backup_root)}</dd></div>
            <div><dt>Manifest</dt><dd>{shortPath($appStore.backups.status?.manifest_path)}</dd></div>
          </dl>
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Permanent originals</p>
              <h3>{$appStore.backups.status?.status ?? "No backup status loaded"}</h3>
            </div>
            <div class="action-row compact">
              <button type="button" class="action-button subtle" disabled={$appStore.backups.loading} on:click={() => void appStore.refreshBackupStatus()}>
                {actionText($appStore.backups.loading, "Refresh status", "Refreshing")}
              </button>
              <button type="button" class="action-button" disabled={$appStore.backups.verifying} on:click={() => void appStore.verifyBackups()}>
                {actionText($appStore.backups.verifying, "Verify backups", "Verifying")}
              </button>
            </div>
          </div>
          {#if $appStore.backups.status}
            <section class="stats-grid compact-stats">
              <article class="metric-mini"><span>Tracked</span><strong>{$appStore.backups.status.tracked_file_count}</strong></article>
              <article class="metric-mini"><span>Missing</span><strong>{$appStore.backups.status.missing_file_count}</strong></article>
              <article class="metric-mini"><span>Mismatched</span><strong>{$appStore.backups.status.mismatched_file_count}</strong></article>
              <article class="metric-mini"><span>Untracked</span><strong>{$appStore.backups.status.untracked_file_count}</strong></article>
            </section>
            <NoticeGroup title="Backup warnings" tone="warning" items={noticesFrom($appStore.backups.status.warnings)} />
            <NoticeGroup title="Backup blockers" tone="danger" items={noticesFrom($appStore.backups.status.blockers)} />
          {/if}
          {#if $appStore.backups.error}
            <section class="inline-alert danger">{$appStore.backups.error}</section>
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "diagnostics"}
      <section class="content-grid wide-right diagnostics-layout">
        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Paths</p>
              <h3>Local diagnostics</h3>
            </div>
            <span class={`status-badge ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
          </div>
          <dl class="detail-list compact-table key-value-table">
            <div><dt>App home</dt><dd class="path-text" title={displayPath($appStore.config?.app_home)}>{displayPath($appStore.config?.app_home)}</dd></div>
            <div><dt>Database path</dt><dd class="path-text" title={displayPath($appStore.config?.database_path)}>{displayPath($appStore.config?.database_path)}</dd></div>
            <div><dt>Configured CookedPCConsole</dt><dd class="path-text" title={displayPath(configuredCookedPath)}>{displayPath(configuredCookedPath)}</dd></div>
            <div><dt>Profile backups</dt><dd class="path-text" title={displayPath(profileBackupRoot)}>{displayPath(profileBackupRoot)}</dd></div>
            <div><dt>Original backups</dt><dd class="path-text" title={displayPath($appStore.backups.status?.backup_root)}>{displayPath($appStore.backups.status?.backup_root)}</dd></div>
            <div><dt>Session logs</dt><dd>In-memory only for the current desktop session</dd></div>
          </dl>
        </article>

        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Counts</p>
              <h3>Current backend summary</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Refresh</button>
          </div>
          <dl class="detail-list compact-table key-value-table">
            <div><dt>Indexed local files</dt><dd>{$appStore.app_status?.local_files_count ?? 0}</dd></div>
            <div><dt>Products</dt><dd>{$appStore.app_status?.product_count ?? 0}</dd></div>
            <div><dt>Titles</dt><dd>{$appStore.app_status?.title_count ?? 0}</dd></div>
            <div><dt>Installed swaps</dt><dd>{$appStore.restore.installed_swaps.length}</dd></div>
            <div><dt>Active swaps</dt><dd>{$appStore.app_status?.active_swap_count ?? 0}</dd></div>
            <div><dt>Permanent originals tracked</dt><dd>{$appStore.backups.status?.tracked_file_count ?? 0}</dd></div>
            <div><dt>Backup status</dt><dd>{backupHealth.label}</dd></div>
            <div><dt>Path posture</dt><dd>{configuredCookedSafety.label}</dd></div>
          </dl>
          <article class={`notice-strip ${configuredCookedSafety.tone}`}>
            <strong>{configuredCookedSafety.label}</strong>
            <p>{configuredCookedSafety.detail}</p>
          </article>
        </article>
      </section>
    {:else if $appStore.active_page === "logs"}
      <section class="content-grid wide-right logs-layout">
        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Expected smoke sequence</p>
              <h3>Commands the GUI should emit</h3>
            </div>
            <span class="status-badge neutral">Operator check</span>
          </div>
          <p>
            During a sandbox click-through, the log should show this command sequence in roughly this order.
          </p>
          <div class="sample-list">
            {#each smokeLogSequence as commandName}
              <span>{commandName}</span>
            {/each}
          </div>
        </article>

        <article class="panel">
          <div class="panel-header">
            <div>
              <p class="panel-label">Recent commands</p>
              <h3>Backend activity console</h3>
            </div>
            <div class="tool-row compact-wrap">
              <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Refresh console</button>
              <button type="button" class="action-button subtle" disabled={$appStore.logs.length === 0} on:click={() => void copyLogsToClipboard()}>
                {logCopyState === "copied" ? "Copied" : logCopyState === "failed" ? "Copy failed" : "Copy log"}
              </button>
            </div>
          </div>
          {#if $appStore.logs.length === 0}
            <p class="empty-state">No backend activity has been recorded yet. Use Game Folder, Database, or Quick Swap to populate the console.</p>
          {:else}
            <div class="monospace-log">
              {#each $appStore.logs as log}
                <article class={`log-entry ${log.kind}`}>
                  <span class="log-timestamp">{formatDate(log.at)}</span>
                  <span class={`log-kind ${log.kind}`}>[{logKindLabel(log.kind)}]</span>
                  <strong>{log.command}</strong>
                  <p>{log.detail}</p>
                </article>
              {/each}
            </div>
          {/if}
        </article>
      </section>
    {/if}
  </main>
</div>
