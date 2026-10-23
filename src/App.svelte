<script lang="ts">
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import LogPanel from "./lib/components/LogPanel.svelte";
  import ModuleWarningDialog from "./lib/components/ModuleWarningDialog.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import Toaster from "./lib/components/Toaster.svelte";
  import { NORMAL, ms } from "./lib/motion";
  import ConsolePage from "./lib/pages/ConsolePage.svelte";
  import KernelPage from "./lib/pages/KernelPage.svelte";
  import LibraryPage from "./lib/pages/LibraryPage.svelte";
  import SettingsPage from "./lib/pages/SettingsPage.svelte";
  import { PAGES, app, consoleStore, logStore, navigation, preferences } from "./lib/state";

  onMount(() => {
    preferences.load();
    void app.load();
    void consoleStore.start();
    void logStore.start();
  });

  /** Ctrl+1..4 switch pages, Ctrl+L toggles the log, F5 looks again. */
  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "F5") {
      event.preventDefault();
      void consoleStore.refresh();
      return;
    }
    if (!event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    if (event.key === "l" || event.key === "L") {
      event.preventDefault();
      preferences.setLogOpen(!preferences.logOpen);
      return;
    }
    const index = Number.parseInt(event.key, 10) - 1;
    const page = PAGES[index];
    if (page) {
      event.preventDefault();
      navigation.go(page.id);
    }
  }
</script>

<svelte:window onkeydown={onkeydown} />

<div class="shell">
  <Sidebar />
  <div class="column">
    <main>
      {#key navigation.page}
        <div class="page" in:fly={{ y: 6, duration: ms(NORMAL), easing: cubicOut }}>
          {#if navigation.page === "console"}
            <ConsolePage />
          {:else if navigation.page === "kernel"}
            <KernelPage />
          {:else if navigation.page === "library"}
            <LibraryPage />
          {:else}
            <SettingsPage />
          {/if}
        </div>
      {/key}
    </main>
    {#if preferences.logOpen}
      <LogPanel />
    {/if}
  </div>
  <StatusBar />
</div>
<Toaster />
<ModuleWarningDialog />

<style>
  .shell {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: 1fr auto;
    grid-template-areas:
      "sidebar main"
      "footer footer";
    height: 100vh;
  }

  .shell > :global(nav) {
    grid-area: sidebar;
  }

  .column {
    grid-area: main;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }

  /* The window is a fixed viewport: the sidebar, this header, the log panel and the status
     bar never move, and the page content area is the only flexible region. It never scrolls
     itself; each page declares which of its regions scroll. */
  main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: var(--space-5) var(--space-6) var(--space-4);
    overflow: hidden;
  }


  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .shell > :global(footer) {
    grid-area: footer;
  }

  @media (max-width: 1023px) {
    main {
      padding: var(--space-4) var(--space-4) var(--space-3);
    }
  }
</style>
