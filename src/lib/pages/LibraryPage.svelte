<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import EmptyState from "../components/library/EmptyState.svelte";
  import FolderTree from "../components/library/FolderTree.svelte";
  import GameDetail from "../components/library/GameDetail.svelte";
  import GameGrid from "../components/library/GameGrid.svelte";
  import ImportPanel from "../components/library/ImportPanel.svelte";
  import SyncBar from "../components/library/SyncBar.svelte";
  import Icon from "../components/Icon.svelte";
  import { FAST, ms } from "../motion";
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

{#if !libraryStore.loaded}
  <div class="empty-note state" aria-busy="true">
    <span class="spinner" aria-hidden="true"></span>
    <p>Reading the library…</p>
  </div>
{:else if libraryStore.error && !libraryStore.view}
  <section class="card failed" aria-labelledby="library-failed">
    <span class="failed-icon"><Icon name="warning" size={24} /></span>
    <div>
      <h2 id="library-failed">The library could not be read.</h2>
      <p class="muted">{libraryStore.error}</p>
    </div>
    <button type="button" class="btn btn-primary" onclick={() => void libraryStore.load()} disabled={libraryStore.loading}>
      <Icon name="refresh" size={16} />
      Try again
    </button>
  </section>
{:else if libraryStore.empty && !libraryStore.importing && !libraryStore.importOpen}
  <EmptyState />
{:else}
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
{/if}

{#if libraryStore.dragOver}
  <div class="drop-overlay" aria-hidden="true" transition:fade={{ duration: ms(FAST) }}>
    <div class="drop-card">Drop ROM files or folders to add them</div>
  </div>
{/if}

<style>
  .state {
    flex: 1;
    justify-content: center;
  }

  .failed {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
    border-left: 5px solid var(--danger);
  }

  .failed div {
    flex: 1;
    min-width: 0;
  }

  .failed-icon {
    display: inline-flex;
    color: var(--danger);
  }

  /* Three full-height columns; only their inner lists scroll. The tree never goes under
     240 px ("Console menu" and "More games..." fit) and the detail panel under 340 px (the
     fields need the room); both grow a little with the window. */
  .library {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(240px, 13vw) minmax(0, 1fr) minmax(340px, 20vw);
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
    display: flex;
    flex-direction: column;
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
    font-weight: var(--weight-semibold);
  }


</style>
