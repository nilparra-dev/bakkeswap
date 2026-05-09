<script lang="ts">
  import type { UiNotice } from "../types";

  export let title: string;
  export let tone: "info" | "warning" | "danger" = "info";
  export let items: UiNotice[] = [];
</script>

{#if items.length > 0}
  <section class={`notice-group ${tone}`}>
    <div class="notice-heading">
      <h3>{title}</h3>
      <span>{items.length}</span>
    </div>
    <ul class="notice-list">
      {#each items as item}
        <li>
          {#if item.code}
            <span class="notice-code">{item.code.split("_").join(" ")}</span>
          {/if}
          <p>{item.message}</p>
        </li>
      {/each}
    </ul>
  </section>
{/if}