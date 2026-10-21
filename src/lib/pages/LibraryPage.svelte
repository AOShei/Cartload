<script lang="ts">
  import { onMount } from "svelte";
  import EmptyState from "../components/library/EmptyState.svelte";
  import FolderTree from "../components/library/FolderTree.svelte";
  import GameDetail from "../components/library/GameDetail.svelte";
  import GameGrid from "../components/library/GameGrid.svelte";
  import ImportPanel from "../components/library/ImportPanel.svelte";
  import SyncBar from "../components/library/SyncBar.svelte";
  import { consoleStore, libraryStore, syncStore } from "../state";

  onMount(() => {
    void libraryStore.start();
    void syncStore.start();
  });

  const report = $derived(consoleStore.report);
  const connected = $derived(report?.status.state === "connected");

  /** The plan follows the console and every change to the games or the tree. */
  $effect(() => {
    const revision = libraryStore.revision;
    void revision;
    if (connected && libraryStore.loaded) {
      syncStore.schedulePlan();
    } else if (report && !connected) {
      syncStore.invalidate();
    }
  });
</script>

{#if libraryStore.empty && !libraryStore.importing && !libraryStore.importOpen}
  <EmptyState />
{:else if libraryStore.loaded || !libraryStore.loading}
  <div class="library">
    <aside class="card tree"><FolderTree /></aside>
    <section class="card centre">
      {#if libraryStore.importOpen}
        <ImportPanel />
      {/if}
      <GameGrid />
    </section>
    <aside class="card detail"><GameDetail /></aside>
    <div class="bar"><SyncBar /></div>
  </div>
{:else}
  <p class="muted">Loading the library…</p>
{/if}

{#if libraryStore.dragOver}
  <div class="drop-overlay" aria-hidden="true">
    <div class="drop-card">Drop ROM files or folders to add them</div>
  </div>
{/if}

<style>
  .library {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 14rem minmax(0, 1fr) 19rem;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: var(--space-3);
  }

  .tree,
  .centre,
  .detail {
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .centre {
    min-width: 0;
  }

  .bar {
    grid-column: 1 / -1;
  }

  .drop-overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 3px dashed var(--accent);
    pointer-events: none;
  }

  .drop-card {
    padding: var(--space-4) var(--space-6);
    border-radius: var(--radius-l);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    font-weight: 600;
  }

  @media (max-width: 1000px) {
    .library {
      grid-template-columns: 12rem minmax(0, 1fr) 16rem;
    }
  }
</style>
