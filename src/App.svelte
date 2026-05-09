<script lang="ts">
  import { onMount } from "svelte";

  import NoticeGroup from "./lib/components/NoticeGroup.svelte";
  import { appStore } from "./lib/store";
  import type { BackupVerificationResult, PageId, SearchHit, UiNotice } from "./lib/types";

  type StatusTone = "ok" | "warn" | "danger" | "neutral";

  interface SafetyInfo {
    tone: StatusTone;
    label: string;
    detail: string;
  }

  const pages: Array<{ id: PageId; label: string; detail: string }> = [
    { id: "home", label: "Home", detail: "Overview and safety posture" },
    { id: "game-folder", label: "Game Folder", detail: "Configure and validate CookedPCConsole" },
    { id: "database", label: "Database", detail: "Import dumps and refresh indexes" },
    { id: "quick-swap", label: "Quick Swap", detail: "TARGET and SOURCE search" },
    { id: "install-preview", label: "Install Preview", detail: "Dry-run install gate" },
    { id: "active-swaps", label: "Active Swaps", detail: "Installed profiles and restore" },
    { id: "backups", label: "Backups", detail: "Permanent originals status" },
    { id: "diagnostics", label: "Diagnostics", detail: "Paths, counts, and local state" },
    { id: "logs", label: "Logs", detail: "Recent backend activity" },
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

  $: selectedSwap =
    $appStore.restore.installed_swaps.find(
      (swap) => swap.profile_name === $appStore.restore.selected_profile_name,
    ) ?? null;

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
  $: pageBadges = {
    home: null,
    "game-folder": configuredCookedPath
      ? $appStore.app_status
        ? `${$appStore.app_status.local_files_count} upk`
        : "Configured"
      : "Needs setup",
    database: $appStore.app_status ? `${$appStore.app_status.product_count} items` : null,
    "quick-swap": $appStore.quick_swap.plan?.profile_name ?? null,
    "install-preview": $appStore.quick_swap.install_preview?.status ?? null,
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
    if (value.length <= 72) {
      return value;
    }
    return `${value.slice(0, 28)}...${value.slice(-36)}`;
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
        label: "Not configured",
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
        label: "Sandbox / project-local",
        detail:
          "This path looks like a fake or project-local sandbox. It is appropriate for Phase 5 sandbox validation.",
      };
    }

    if (
      liveInstallHints.some((hint) => normalized.includes(hint)) ||
      (normalized.includes("/rocketleague/") && !normalized.includes("/target/gui_smoke/"))
    ) {
      return {
        tone: "danger",
        label: "Possible live install",
        detail:
          "This path resembles a real Rocket League install. Keep install and restore on copied or fake roots until you intentionally leave sandbox validation.",
      };
    }

    return {
      tone: "warn",
      label: "Local custom path",
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
        label: "Select TARGET and SOURCE",
        detail:
          "Choose the item you already own or equip for TARGET, then the item you want to see for SOURCE.",
      };
    }
    if (!target || !source) {
      return {
        tone: "warn",
        label: "Selection incomplete",
        detail: "Both TARGET and SOURCE must be selected before the backend can create a plan.",
      };
    }
    if (target.slot && source.slot && target.slot === source.slot) {
      return {
        tone: "ok",
        label: "Same slot metadata",
        detail:
          "The selected items share the same slot label. The backend plan still remains the source of truth for compatibility.",
      };
    }
    if (target.slot && source.slot && target.slot !== source.slot) {
      return {
        tone: "danger",
        label: "Different slot metadata",
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
        label: "No backup report loaded",
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
        label: "Backup attention required",
        detail:
          "Some tracked originals are missing or mismatched. Review blockers before using restore workflows.",
      };
    }
    if (status.untracked_file_count > 0 || status.warnings.length > 0) {
      return {
        tone: "warn",
        label: "Backup review recommended",
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
    <div class="brand">
      <div>
        <p class="eyebrow">BakkesSwap</p>
        <h1>Desktop Control Room</h1>
      </div>
      <p class="sidebar-copy">
        TARGET is the item you already own or equip. SOURCE is the item you want to see locally.
        All plan, build, install, restore, and backup logic stays in Rust.
      </p>
    </div>

    <nav class="nav">
      {#each pages as page}
        <button
          type="button"
          class:active={page.id === $appStore.active_page}
          aria-pressed={page.id === $appStore.active_page}
          on:click={() => appStore.setActivePage(page.id)}
        >
          <span>{page.label}</span>
          {#if pageBadges[page.id]}
            <small>{pageBadges[page.id]}</small>
          {/if}
        </button>
      {/each}
    </nav>

    <section class="sidebar-panel">
      <div class="sidebar-panel-heading">
        <h2>Bridge</h2>
        <span class={`status-chip ${$appStore.tauri_available ? "ok" : "danger"}`}>
          {$appStore.tauri_available ? "Tauri ready" : "Browser only"}
        </span>
      </div>
      <p>
        {$appStore.tauri_available
          ? "Frontend actions call bakkeswap-core through Tauri commands. The GUI does not reimplement planner or installer logic."
          : "Run with npm run tauri:dev to enable the Rust backend and folder picker."}
      </p>
    </section>

    <section class="sidebar-panel">
      <div class="sidebar-panel-heading">
        <h2>Current path</h2>
        <span class={`status-chip ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
      </div>
      <p class="path-value">{shortPath(configuredCookedPath)}</p>
      <p class="context-note">{configuredCookedSafety.detail}</p>
    </section>

    <section class="sidebar-panel">
      <div class="sidebar-panel-heading">
        <h2>Guardrails</h2>
        <span class="status-chip warn">Non-negotiable</span>
      </div>
      <ul class="rule-list">
        {#each safetyRules as rule}
          <li>{rule}</li>
        {/each}
      </ul>
    </section>
  </aside>

  <main class="content">
    <section class="hero">
      <div>
        <p class="eyebrow">Phase 5C</p>
        <h2>Human GUI click-through polish</h2>
        <p>
          This pass compares the real Tauri window against the sandbox smoke workflow in
          <strong>target/gui_smoke</strong>. The frontend stays thin while small live UX rough edges
          are tightened without moving backend logic out of Rust.
        </p>
      </div>

      <div class="hero-stack">
        <article class="hero-card">
          <span>Current page</span>
          <strong>{pages.find((page) => page.id === $appStore.active_page)?.label}</strong>
          <p>{pages.find((page) => page.id === $appStore.active_page)?.detail}</p>
        </article>
        <article class="hero-card">
          <span>Configured CookedPCConsole</span>
          <strong>{configuredCookedSafety.label}</strong>
          <p class="path-value">{shortPath(configuredCookedPath)}</p>
        </article>
      </div>
    </section>

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
          <button type="button" class="action-button subtle" on:click={() => appStore.setActivePage("logs")}>Open logs</button>
        </div>
        {#if $appStore.logs.length === 0}
          <p class="empty-state">No backend activity yet.</p>
        {:else}
          <div class="log-list compact">
            {#each $appStore.logs.slice(0, 6) as log}
              <article class={`log-entry ${log.kind}`}>
                <div>
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
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseGameFolder()}>Browse</button>
            <button type="button" class="action-button" disabled={$appStore.setup.validating || !$appStore.setup.game_path_input.trim()} on:click={() => void appStore.validateCurrentGamePath()}>
              {actionText($appStore.setup.validating, "Validate path", "Validating")}
            </button>
            <button type="button" class="action-button accent" disabled={!gamePathReadyToSave || $appStore.setup.saving} on:click={() => void appStore.saveCurrentGamePath()}>
              {actionText($appStore.setup.saving, "Save validated path", "Saving")}
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
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseCodeRedFolder()}>Browse</button>
            <button type="button" class="action-button accent" disabled={!$appStore.database.import_folder_input.trim() || $appStore.database.importing} on:click={() => void appStore.importCurrentCodeRedFolder()}>
              {actionText($appStore.database.importing, "Import dumps", "Importing")}
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
              {actionText($appStore.database.refreshing, "Refresh DB", "Refreshing")}
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
      <section class="content-grid two-up">
        <article class="panel context-card">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Risk context</p>
              <h3>{configuredCookedSafety.label}</h3>
            </div>
            <span class={`status-chip ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
          </div>
          <p>{shortPath(configuredCookedPath)}</p>
          <p class="context-note">{configuredCookedSafety.detail}</p>
        </article>

        <article class={`panel preflight-card ${selectionReadiness.tone}`}>
          <div class="panel-heading">
            <div>
              <p class="panel-label">Selection preflight</p>
              <h3>{selectionReadiness.label}</h3>
            </div>
            <span class={`status-chip ${selectionReadiness.tone}`}>{selectionReadiness.label}</span>
          </div>
          <p>{selectionReadiness.detail}</p>
        </article>
      </section>

      <section class="content-grid two-up">
        <article class="panel search-panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">TARGET</p>
              <h3>Item you own or equip</h3>
            </div>
            <span class="status-chip neutral">Products only</span>
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
              <p class="empty-state">Searching TARGET items...</p>
            {:else if $appStore.quick_swap.target.results.length === 0}
              <p class="empty-state">Type a query to load TARGET candidates.</p>
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
        </article>

        <article class="panel search-panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">SOURCE</p>
              <h3>Item you want to see</h3>
            </div>
            <span class="status-chip neutral">Products only</span>
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
              <p class="empty-state">Searching SOURCE items...</p>
            {:else if $appStore.quick_swap.source.results.length === 0}
              <p class="empty-state">Type a query to load SOURCE candidates.</p>
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
        </article>
      </section>

      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="panel-label">Plan and build</p>
            <h3>{$appStore.quick_swap.plan?.profile_name ?? "No swap plan yet"}</h3>
          </div>
          <div class="action-row compact">
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.target.selected || !$appStore.quick_swap.source.selected || $appStore.quick_swap.creating_plan} on:click={() => void appStore.createCurrentPlan()}>
              {actionText($appStore.quick_swap.creating_plan, "Create backend plan", "Planning")}
            </button>
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.plan || ($appStore.quick_swap.plan?.build_blockers.length ?? 0) > 0 || $appStore.quick_swap.building} on:click={() => void appStore.buildCurrentPlan()}>
              {actionText($appStore.quick_swap.building, "Build plan", "Building")}
            </button>
            <button type="button" class="action-button accent" disabled={!$appStore.quick_swap.build_report || $appStore.quick_swap.build_report.status !== "built" || $appStore.quick_swap.previewing_install} on:click={() => void appStore.previewCurrentInstall()}>
              {actionText($appStore.quick_swap.previewing_install, "Open install preview", "Preparing preview")}
            </button>
          </div>
        </div>

        <article class={`preflight-card ${selectionReadiness.tone}`}>
          <strong>{selectionReadiness.label}</strong>
          <p>{selectionReadiness.detail}</p>
        </article>

        {#if $appStore.quick_swap.plan}
          <dl class="detail-list compact gridish">
            <div><dt>Profile</dt><dd>{$appStore.quick_swap.plan.profile_name}</dd></div>
            <div><dt>Backend compatibility</dt><dd>{$appStore.quick_swap.plan.compatibility.same_slot ? "Same slot confirmed" : "Slot mismatch"}</dd></div>
            <div><dt>TARGET</dt><dd>{$appStore.quick_swap.plan.target_product.name}</dd></div>
            <div><dt>SOURCE</dt><dd>{$appStore.quick_swap.plan.source_product.name}</dd></div>
            <div><dt>Configured CookedPCConsole</dt><dd>{shortPath($appStore.quick_swap.plan.configured_cooked_root)}</dd></div>
            <div><dt>Plan file</dt><dd>{shortPath($appStore.quick_swap.plan.plan_path)}</dd></div>
          </dl>
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
      <section class="content-grid two-up">
        <article class="panel context-card">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Risk context</p>
              <h3>{installCookedSafety.label}</h3>
            </div>
            <span class={`status-chip ${installCookedSafety.tone}`}>{installCookedSafety.label}</span>
          </div>
          <p>{shortPath(installCookedPath)}</p>
          <p class="context-note">{installCookedSafety.detail}</p>
        </article>

        <article class="panel context-card">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Confirmation gate</p>
              <h3>{$appStore.quick_swap.install_preview?.confirmation_phrase ?? "Generate preview first"}</h3>
            </div>
            <span class={`status-chip ${installPhraseMatches ? "ok" : "warn"}`}>{installPhraseMatches ? "Phrase matched" : "Exact phrase required"}</span>
          </div>
          <p>
            Install remains disabled until preview is ready, blockers are empty, and the backend-issued phrase is typed exactly.
          </p>
        </article>
      </section>

      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="panel-label">Install gate</p>
            <h3>{$appStore.quick_swap.install_preview?.profile_name ?? $appStore.quick_swap.plan?.profile_name ?? "No plan selected"}</h3>
          </div>
          <span class={`status-chip ${installPreviewReady ? "ok" : "warn"}`}>
            {$appStore.quick_swap.install_preview?.status ?? "waiting"}
          </span>
        </div>
        {#if !$appStore.quick_swap.plan}
          <p class="empty-state">Create and build a plan on the Quick Swap page first.</p>
        {:else}
          <dl class="detail-list compact gridish">
            <div><dt>Configured CookedPCConsole</dt><dd>{shortPath(installCookedPath)}</dd></div>
            <div><dt>Path posture</dt><dd>{installCookedSafety.label}</dd></div>
            <div><dt>Plan file</dt><dd>{shortPath($appStore.quick_swap.plan.plan_path)}</dd></div>
            <div><dt>Build status</dt><dd>{$appStore.quick_swap.build_report?.status ?? "Not built"}</dd></div>
          </dl>
          <div class="action-row">
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.build_report || $appStore.quick_swap.build_report.status !== "built" || $appStore.quick_swap.previewing_install} on:click={() => void appStore.previewCurrentInstall()}>
              {actionText($appStore.quick_swap.previewing_install, "Generate install preview", "Preparing preview")}
            </button>
          </div>
          {#if $appStore.quick_swap.install_preview}
            <div class="table-like">
              {#each $appStore.quick_swap.install_preview.files as file}
                <article>
                  <div><strong>{file.kind}</strong><p>{file.target_filename}</p></div>
                  <div><span>{file.would_overwrite ? "Would overwrite" : "Fresh copy"}</span><small>{shortPath(file.target_path)}</small></div>
                </article>
              {/each}
            </div>
            <NoticeGroup title="Preview warnings" tone="warning" items={noticesFrom($appStore.quick_swap.install_preview.warnings)} />
            <NoticeGroup title="Preview blockers" tone="danger" items={noticesFrom($appStore.quick_swap.install_preview.blockers)} />
            <article class="confirm-card">
              <div>
                <p class="panel-label">Typed confirmation</p>
                <h4 class="confirmation-phrase">{$appStore.quick_swap.install_preview.confirmation_phrase}</h4>
                <p>Type the exact phrase below. Install remains disabled until the phrase matches exactly.</p>
              </div>
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
          {/if}
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
      <section class="content-grid wide-right">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Installed profiles</p>
              <h3>Restore targets</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Reload list</button>
          </div>
          {#if $appStore.restore.installed_swaps.length === 0}
            <p class="empty-state">No installed swap records yet.</p>
          {:else}
            <div class="result-list installed-list">
              {#each $appStore.restore.installed_swaps as swap}
                <button type="button" class:selected={$appStore.restore.selected_profile_name === swap.profile_name} on:click={() => appStore.selectInstalledProfile(swap.profile_name)}>
                  <div><strong>{swap.profile_name}</strong><p>{swap.target_name ?? "Unknown target"} -> {swap.source_name ?? "Unknown source"}</p></div>
                  <div><span>{swap.active ? "Active" : "Inactive"}</span><small>{formatDate(swap.installed_at)}</small></div>
                </button>
              {/each}
            </div>
          {/if}
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Restore</p>
              <h3>{selectedSwap?.profile_name ?? "Choose a profile"}</h3>
            </div>
            {#if selectedSwap}
              <span class={`status-chip ${selectedSwap.active ? "ok" : "warn"}`}>{selectedSwap.active ? "Active install" : "Historical record"}</span>
            {/if}
          </div>
          {#if !selectedSwap}
            <p class="empty-state">Select an installed profile to preview restore steps.</p>
          {:else}
            <dl class="detail-list compact gridish">
              <div><dt>TARGET</dt><dd>{selectedSwap.target_name ?? "Unknown"}</dd></div>
              <div><dt>SOURCE</dt><dd>{selectedSwap.source_name ?? "Unknown"}</dd></div>
              <div><dt>Selected install root</dt><dd>{shortPath(selectedSwap.cooked_root)}</dd></div>
              <div><dt>Configured CookedPCConsole</dt><dd>{shortPath(configuredCookedPath)}</dd></div>
              <div><dt>Configured path posture</dt><dd>{restoreCookedSafety.label}</dd></div>
              <div><dt>Plan status</dt><dd>{selectedSwap.plan_status ?? "Unknown"}</dd></div>
            </dl>
            <article class={`preflight-card ${restoreCookedSafety.tone}`}>
              <strong>{restoreCookedSafety.label}</strong>
              <p>{restoreCookedSafety.detail}</p>
            </article>
            <label class="checkbox-field danger-toggle">
              <input type="checkbox" checked={$appStore.restore.from_originals} on:change={(event) => appStore.setRestoreFromOriginals(inputChecked(event))} />
              <span>Emergency mode: restore from permanent originals instead of the profile backup</span>
            </label>
            <div class="action-row">
              <button type="button" class="action-button" disabled={$appStore.restore.previewing} on:click={() => void appStore.previewCurrentRestore()}>
                {actionText($appStore.restore.previewing, "Preview restore", "Preparing preview")}
              </button>
            </div>
            {#if $appStore.restore.preview}
              <div class="table-like">
                {#each $appStore.restore.preview.files as file}
                  <article>
                    <div><strong>{file.kind}</strong><p>{file.backup_kind}</p></div>
                    <div><span>{file.destination_exists ? "Destination exists" : "Destination missing"}</span><small>{shortPath(file.destination_path)}</small></div>
                  </article>
                {/each}
              </div>
              <NoticeGroup title="Restore warnings" tone="warning" items={noticesFrom($appStore.restore.preview.warnings)} />
              <NoticeGroup title="Restore blockers" tone="danger" items={noticesFrom($appStore.restore.preview.blockers)} />
              <article class="confirm-card">
                <div>
                  <p class="panel-label">Typed confirmation</p>
                  <h4 class="confirmation-phrase">{$appStore.restore.preview.confirmation_phrase}</h4>
                  <p>Restore remains disabled until the preview is ready and the exact phrase is typed.</p>
                </div>
                <label class="field">
                  <span>Confirmation phrase</span>
                  <input type="text" value={$appStore.restore.confirmation} placeholder="Type the restore confirmation phrase" on:input={(event) => appStore.setRestoreConfirmation(inputValue(event))} />
                </label>
                <button type="button" class="action-button accent" disabled={!restorePreviewReady || !restorePhraseMatches || $appStore.restore.restoring} on:click={() => void appStore.restoreCurrentProfile()}>
                  {actionText($appStore.restore.restoring, "Restore confirmed", "Restoring")}
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
      <section class="content-grid two-up">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Paths</p>
              <h3>Local diagnostics</h3>
            </div>
            <span class={`status-chip ${configuredCookedSafety.tone}`}>{configuredCookedSafety.label}</span>
          </div>
          <dl class="detail-list">
            <div><dt>App home</dt><dd>{shortPath($appStore.config?.app_home)}</dd></div>
            <div><dt>Database path</dt><dd>{shortPath($appStore.config?.database_path)}</dd></div>
            <div><dt>Configured CookedPCConsole</dt><dd>{shortPath(configuredCookedPath)}</dd></div>
            <div><dt>Profile backups</dt><dd>{shortPath(profileBackupRoot)}</dd></div>
            <div><dt>Original backups</dt><dd>{shortPath($appStore.backups.status?.backup_root)}</dd></div>
            <div><dt>Session logs</dt><dd>In-memory only for the current desktop session</dd></div>
          </dl>
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Counts</p>
              <h3>Current backend summary</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Refresh</button>
          </div>
          <dl class="detail-list">
            <div><dt>Indexed local files</dt><dd>{$appStore.app_status?.local_files_count ?? 0}</dd></div>
            <div><dt>Products</dt><dd>{$appStore.app_status?.product_count ?? 0}</dd></div>
            <div><dt>Titles</dt><dd>{$appStore.app_status?.title_count ?? 0}</dd></div>
            <div><dt>Installed swaps</dt><dd>{$appStore.restore.installed_swaps.length}</dd></div>
            <div><dt>Active swaps</dt><dd>{$appStore.app_status?.active_swap_count ?? 0}</dd></div>
            <div><dt>Permanent originals tracked</dt><dd>{$appStore.backups.status?.tracked_file_count ?? 0}</dd></div>
          </dl>
          <article class={`preflight-card ${configuredCookedSafety.tone}`}>
            <strong>{configuredCookedSafety.label}</strong>
            <p>{configuredCookedSafety.detail}</p>
          </article>
        </article>
      </section>
    {:else if $appStore.active_page === "logs"}
      <section class="content-grid two-up">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Expected smoke sequence</p>
              <h3>Commands the GUI should emit</h3>
            </div>
            <span class="status-chip neutral">Manual check</span>
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
          <div class="panel-heading">
            <div>
              <p class="panel-label">Recent commands</p>
              <h3>Backend activity log</h3>
            </div>
            <button type="button" class="action-button subtle" on:click={() => void appStore.refreshOverview(false)}>Refresh summaries</button>
          </div>
          {#if $appStore.logs.length === 0}
            <p class="empty-state">No backend activity has been recorded yet.</p>
          {:else}
            <div class="log-list">
              {#each $appStore.logs as log}
                <article class={`log-entry ${log.kind}`}>
                  <div><strong>{log.command}</strong><p>{log.detail}</p></div>
                  <span>{formatDate(log.at)}</span>
                </article>
              {/each}
            </div>
          {/if}
        </article>
      </section>
    {/if}
  </main>
</div>
