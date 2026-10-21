<script lang="ts">
  import Sidebar from "./lib/Sidebar.svelte";
  import StatusBar from "./lib/StatusBar.svelte";
  import type { Section } from "./lib/sections";
  import { fetchAppVersion, type AppVersion } from "./lib/version";

  let current = $state<Section>("Console");
  let version = $state<AppVersion | null>(null);
  let versionError = $state<string | null>(null);

  $effect(() => {
    fetchAppVersion()
      .then((v) => {
        version = v;
      })
      .catch((err: unknown) => {
        versionError = err instanceof Error ? err.message : String(err);
      });
  });
</script>

<div class="shell">
  <Sidebar {current} onselect={(section) => (current = section)} />
  <main>
    <h1>{current}</h1>
  </main>
  <StatusBar {version} error={versionError} />
</div>

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

  main {
    grid-area: main;
    padding: 1.5rem 2rem;
    overflow: auto;
  }

  .shell > :global(footer) {
    grid-area: footer;
  }

  h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }
</style>
