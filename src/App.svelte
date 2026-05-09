<script lang="ts">
  import { onMount } from "svelte";

  import NoticeGroup from "./lib/components/NoticeGroup.svelte";
  import { appStore } from "./lib/store";
  import type { PageId, SearchHit, UiNotice } from "./lib/types";

  const pages: Array<{ id: PageId; label: string; detail: string }> = [
    { id: "home", label: "Home", detail: "Overview and safety posture" },
    { id: "game-folder", label: "Game Folder", detail: "Configure and validate CookedPCConsole" },
    { id: "database", label: "Database", detail: "Import dumps and refresh indexes" },
    { id: "quick-swap", label: "Quick Swap", detail: "TARGET and SOURCE search" },
    { id: "install-preview", label: "Install Preview", detail: "Dry-run install gate" },
    { id: "active-swaps", label: "Active Swaps", detail: "Installed profiles and restore" },
    { id: "backups", label: "Backups", detail: "Permanent originals status" },
    { id: "logs", label: "Logs", detail: "Recent backend activity" },
  ];

  const safetyRules = [
    "Offline and local only. Do not use this tool online and do not bypass EAC.",
    "Sandbox-first bring-up. Validate against copied or fake CookedPCConsole roots before touching a live install.",
    "Planning, rebuild, install, and restore logic stay in Rust. The frontend only orchestrates backend contracts.",
  ];

  onMount(() => {
    void appStore.load();
  });

  $: selectedSwap =
    $appStore.restore.installed_swaps.find(
      (swap) => swap.profile_name === $appStore.restore.selected_profile_name,
    ) ?? null;

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

  function pageBadge(page: PageId): string | null {
    switch (page) {
      case "game-folder":
        return $appStore.app_status?.configured_cooked_dir
          ? `${$appStore.app_status.local_files_count} upk`
          : "Needs setup";
      case "database":
        return $appStore.app_status ? `${$appStore.app_status.product_count} items` : null;
      case "quick-swap":
        return $appStore.quick_swap.plan?.profile_name ?? null;
      case "install-preview":
        return $appStore.quick_swap.install_preview?.status ?? null;
      case "active-swaps":
        return `${$appStore.app_status?.active_swap_count ?? 0} active`;
      case "backups":
        return $appStore.backups.status
          ? `${$appStore.backups.status.tracked_file_count} tracked`
          : null;
      case "logs":
        return `${$appStore.logs.length}`;
      default:
        return null;
    }
  }

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

  function looksLikeRealInstall(path: string | null | undefined): boolean {
    const value = path?.toLowerCase() ?? "";
    return value.includes("rocketleague") || value.includes("steamapps") || value.includes("epic games");
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
      <p class="eyebrow">BakkesSwap</p>
      <h1>Desktop Control Room</h1>
      <p class="sidebar-copy">
        TARGET is the item you already own or equip in Rocket League. SOURCE is the item you want to
        see locally. All planning and file mutation stays in Rust.
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
          {#if pageBadge(page.id)}
            <small>{pageBadge(page.id)}</small>
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
          ? "Frontend actions call the existing bakkeswap-core services through Tauri commands."
          : "Run with npm run tauri:dev to enable the Rust backend and folder picker."}
      </p>
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
        <p class="eyebrow">Phase 5</p>
        <h2>First usable desktop flow over the stable backend</h2>
        <p>
          Search, plan, build, install preview, confirmed install, backup verification, and restore
          all route through the Rust backend contracts. The GUI is intentionally thin.
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
          <strong>{looksLikeRealInstall($appStore.config?.cooked_dir) ? "Looks like a live install" : "Sandbox or unknown"}</strong>
          <p>{shortPath($appStore.config?.cooked_dir)}</p>
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
        <p>Reading status, config, installed swaps, and backup verification from the Rust backend.</p>
      </section>
    {:else if $appStore.active_page === "home"}
      <section class="stats-grid">
        <article class="panel metric-card">
          <p class="panel-label">Cooked path</p>
          <strong>{shortPath($appStore.app_status?.configured_cooked_dir)}</strong>
          <p>{$appStore.app_status?.local_files_count ?? 0} indexed .upk files</p>
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
          <p>{$appStore.backups.status?.missing_file_count ?? 0} missing</p>
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
            <div><dt>Game path</dt><dd>{shortPath($appStore.config?.game_path_input)}</dd></div>
            <div><dt>CookedPCConsole</dt><dd>{shortPath($appStore.config?.cooked_dir)}</dd></div>
            <div><dt>CodeRed dumps</dt><dd>{shortPath($appStore.config?.codered_dumps_dir)}</dd></div>
            <div><dt>App home</dt><dd>{shortPath($appStore.config?.app_home)}</dd></div>
          </dl>
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Safety posture</p>
              <h3>Configured path check</h3>
            </div>
            <span class={`status-chip ${looksLikeRealInstall($appStore.config?.cooked_dir) ? "danger" : "ok"}`}>
              {looksLikeRealInstall($appStore.config?.cooked_dir) ? "Live install risk" : "Sandbox first"}
            </span>
          </div>
          <p>
            If this looks like a real Rocket League install, keep GUI smoke checks on a copied or
            fake CookedPCConsole root until you intentionally move beyond sandbox validation.
          </p>
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
          <button type="button" class="action-button subtle" on:click={() => appStore.setActivePage("logs")}>
            Open logs
          </button>
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
            <span class={`status-chip ${$appStore.setup.validation?.is_valid ? "ok" : "warn"}`}>
              {$appStore.setup.validation?.is_valid ? "Valid" : "Needs validation"}
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
          <div class="action-row">
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseGameFolder()}>Browse</button>
            <button type="button" class="action-button" disabled={$appStore.setup.validating} on:click={() => void appStore.validateCurrentGamePath()}>
              {actionText($appStore.setup.validating, "Validate path", "Validating")}
            </button>
            <button type="button" class="action-button accent" disabled={$appStore.setup.saving} on:click={() => void appStore.saveCurrentGamePath()}>
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
              <h3>Normalized CookedPCConsole target</h3>
            </div>
            <span class={`status-chip ${looksLikeRealInstall($appStore.setup.validation?.normalized_cooked_dir) ? "danger" : "ok"}`}>
              {looksLikeRealInstall($appStore.setup.validation?.normalized_cooked_dir) ? "Live install risk" : "Safe or unknown"}
            </span>
          </div>
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
              <h3>Import dump metadata</h3>
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
          <div class="action-row">
            <button type="button" class="action-button subtle" on:click={() => void appStore.browseCodeRedFolder()}>Browse</button>
            <button type="button" class="action-button accent" disabled={$appStore.database.importing} on:click={() => void appStore.importCurrentCodeRedFolder()}>
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
        </article>

        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">Refresh</p>
              <h3>Re-import dumps and refresh local .upk index</h3>
            </div>
            <button type="button" class="action-button" disabled={$appStore.database.refreshing} on:click={() => void appStore.refreshDatabase()}>
              {actionText($appStore.database.refreshing, "Refresh DB", "Refreshing")}
            </button>
          </div>
          <dl class="detail-list">
            <div><dt>Products</dt><dd>{$appStore.app_status?.product_count ?? 0}</dd></div>
            <div><dt>Titles</dt><dd>{$appStore.app_status?.title_count ?? 0}</dd></div>
            <div><dt>Local .upk indexed</dt><dd>{$appStore.app_status?.local_files_count ?? 0}</dd></div>
          </dl>
          {#if $appStore.database.last_refresh_result}
            <NoticeGroup title="Refresh warnings" tone="warning" items={noticesFrom($appStore.database.last_refresh_result.warnings)} />
          {/if}
          {#if $appStore.database.error}
            <section class="inline-alert danger">{$appStore.database.error}</section>
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "quick-swap"}
      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="panel-label">Selection language</p>
            <h3>Keep TARGET and SOURCE unambiguous</h3>
          </div>
          <span class="status-chip warn">Top 50 results</span>
        </div>
        <p>
          TARGET is the owned or equipped item. SOURCE is the desired local appearance. Search is
          debounced and capped in the backend.
        </p>
      </section>

      <section class="content-grid two-up">
        <article class="panel search-panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">TARGET</p>
              <h3>Owned or equipped item</h3>
            </div>
            <span class="status-chip neutral">Products only</span>
          </div>
          <label class="field">
            <span>Search target</span>
            <input type="text" value={$appStore.quick_swap.target.query} placeholder="Search product id, name, slot, or package" on:input={(event) => appStore.setSearchQuery("target", inputValue(event))} />
          </label>
          <div class="result-list">
            {#if $appStore.quick_swap.target.loading}
              <p class="empty-state">Searching target items...</p>
            {:else if $appStore.quick_swap.target.results.length === 0}
              <p class="empty-state">Type a query to load target candidates.</p>
            {:else}
              {#each $appStore.quick_swap.target.results as hit}
                <button type="button" class:selected={$appStore.quick_swap.target.selected?.id === hit.id} class:disabled={!canSelect(hit)} disabled={!canSelect(hit)} on:click={() => appStore.selectSearchHit("target", hit)}>
                  <div><strong>{hit.name}</strong><p>#{hit.id}</p></div>
                  <div><span>{hit.slot ?? "Unknown slot"}</span><small>{hit.quality ?? hit.note ?? "No quality metadata"}</small></div>
                </button>
              {/each}
            {/if}
          </div>
        </article>

        <article class="panel search-panel">
          <div class="panel-heading">
            <div>
              <p class="panel-label">SOURCE</p>
              <h3>Desired local appearance</h3>
            </div>
            <span class="status-chip neutral">Products only</span>
          </div>
          <label class="field">
            <span>Search source</span>
            <input type="text" value={$appStore.quick_swap.source.query} placeholder="Search source product id, name, slot, or package" on:input={(event) => appStore.setSearchQuery("source", inputValue(event))} />
          </label>
          <div class="result-list">
            {#if $appStore.quick_swap.source.loading}
              <p class="empty-state">Searching source items...</p>
            {:else if $appStore.quick_swap.source.results.length === 0}
              <p class="empty-state">Type a query to load source candidates.</p>
            {:else}
              {#each $appStore.quick_swap.source.results as hit}
                <button type="button" class:selected={$appStore.quick_swap.source.selected?.id === hit.id} class:disabled={!canSelect(hit)} disabled={!canSelect(hit)} on:click={() => appStore.selectSearchHit("source", hit)}>
                  <div><strong>{hit.name}</strong><p>#{hit.id}</p></div>
                  <div><span>{hit.slot ?? "Unknown slot"}</span><small>{hit.quality ?? hit.note ?? "No quality metadata"}</small></div>
                </button>
              {/each}
            {/if}
          </div>
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
              {actionText($appStore.quick_swap.creating_plan, "Create plan", "Planning")}
            </button>
            <button type="button" class="action-button" disabled={!$appStore.quick_swap.plan || ($appStore.quick_swap.plan?.build_blockers.length ?? 0) > 0 || $appStore.quick_swap.building} on:click={() => void appStore.buildCurrentPlan()}>
              {actionText($appStore.quick_swap.building, "Build plan", "Building")}
            </button>
            <button type="button" class="action-button accent" disabled={!$appStore.quick_swap.build_report || $appStore.quick_swap.build_report.status !== "built" || $appStore.quick_swap.previewing_install} on:click={() => void appStore.previewCurrentInstall()}>
              {actionText($appStore.quick_swap.previewing_install, "Install preview", "Preparing preview")}
            </button>
          </div>
        </div>
        {#if $appStore.quick_swap.plan}
          <dl class="detail-list compact gridish">
            <div><dt>Profile</dt><dd>{$appStore.quick_swap.plan.profile_name}</dd></div>
            <div><dt>Same slot</dt><dd>{$appStore.quick_swap.plan.compatibility.same_slot ? "Yes" : "No"}</dd></div>
            <div><dt>TARGET</dt><dd>{$appStore.quick_swap.plan.target_product.name}</dd></div>
            <div><dt>SOURCE</dt><dd>{$appStore.quick_swap.plan.source_product.name}</dd></div>
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
                <h4>{$appStore.quick_swap.install_preview.confirmation_phrase}</h4>
                <p>Install stays disabled until the phrase matches exactly and the preview is ready.</p>
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
              <div><dt>Plan status</dt><dd>{selectedSwap.plan_status ?? "Unknown"}</dd></div>
              <div><dt>Cooked root</dt><dd>{shortPath(selectedSwap.cooked_root)}</dd></div>
            </dl>
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
                  <h4>{$appStore.restore.preview.confirmation_phrase}</h4>
                  <p>Restore stays disabled until the preview is ready and the phrase is typed exactly.</p>
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
            {/if}
            {#if $appStore.restore.error}
              <section class="inline-alert danger">{$appStore.restore.error}</section>
            {/if}
          {/if}
        </article>
      </section>
    {:else if $appStore.active_page === "backups"}
      <section class="panel">
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
      </section>
    {:else if $appStore.active_page === "logs"}
      <section class="panel">
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
      </section>
    {/if}
  </main>
</div>