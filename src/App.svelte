<script lang="ts">
  const pages = [
    "Home",
    "Game Folder",
    "Quick Swap",
    "Active Swaps",
    "Backups",
    "Database",
    "Settings",
    "Logs",
  ] as const;

  type Page = (typeof pages)[number];

  let activePage: Page = "Home";

  const milestones = [
    "Rust CLI parity with proven Python behavior",
    "SQLite-backed local metadata and status tracking",
    "Safe dry-run install previews before any real overwrite",
    "Tauri desktop shell after CLI milestone is stable",
  ];

  const boundaries = [
    "Local and offline only",
    "No EAC bypass or runtime memory editing",
    "No server inventory changes",
    "No Player Title runtime module in v1",
    "Raw rename and raw copy are never the default method",
  ];
</script>

<svelte:head>
  <title>BakkesSwap</title>
  <meta name="description" content="Rust and Tauri rewrite of the proven BakkesSwap prototype." />
</svelte:head>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">BS</div>
      <div>
        <h1>BakkesSwap</h1>
        <p>v1 rewrite workspace</p>
      </div>
    </div>

    <nav>
      {#each pages as page}
        <button
          class:active={page === activePage}
          on:click={() => (activePage = page)}
          type="button"
        >
          {page}
        </button>
      {/each}
    </nav>
  </aside>

  <main class="content">
    <section class="hero">
      <div>
        <p class="eyebrow">CLI first</p>
        <h2>{activePage}</h2>
        <p>
          This shell is the desktop surface for the Rust rewrite. The Python prototype remains the
          reference implementation until the CLI reaches golden-test parity.
        </p>
      </div>
      <div class="status-card">
        <span>Rewrite state</span>
        <strong>Skeleton only</strong>
        <small>No real install logic ported yet</small>
      </div>
    </section>

    <section class="grid">
      <article class="panel">
        <h3>Milestones</h3>
        <ul>
          {#each milestones as item}
            <li>{item}</li>
          {/each}
        </ul>
      </article>

      <article class="panel accent">
        <h3>Safety Boundaries</h3>
        <ul>
          {#each boundaries as item}
            <li>{item}</li>
          {/each}
        </ul>
      </article>

      <article class="panel">
        <h3>Golden Validation</h3>
        <p>
          The rewrite must preserve the proven target-identity rebuild path, dry-run install
          previews, permanent original backups, per-profile backups, and real game-path handling.
        </p>
      </article>
    </section>
  </main>
</div>
