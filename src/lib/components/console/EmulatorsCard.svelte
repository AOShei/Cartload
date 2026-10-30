<script lang="ts">
  import { untrack } from "svelte";
  import { consoleStore, flowStore, formatBytes, modulesStore, withRetroarch } from "../../state";
  import Icon from "../Icon.svelte";
  import { canManage } from "./watch";

  /* RetroArch and its cores on the console: install, update (when the Mod Hub has a build
     the console does not), remove (refused while games need them), with the download
     progress in the row and the rest in the progress window. */
  const ready = $derived(canManage(consoleStore.report));

  /** Reads the modules again once a task has ended and the console is back (an install or
   *  a removal reboots it). */
  $effect(() => {
    if (ready && !flowStore.running) {
      untrack(() => void modulesStore.load());
    }
  });

  /** What the row says about the module's state. */
  function state(module: { installed: boolean | null; cached: boolean; approx_bytes: number }): string {
    if (module.installed === true) {
      return "Installed";
    }
    return module.cached ? "Downloaded" : formatBytes(module.approx_bytes);
  }
</script>

<section class="card emulators" aria-labelledby="emulators-heading">
  <div class="head">
    <h2 id="emulators-heading">Emulators</h2>
    {#if modulesStore.loading}
      <span class="spinner" aria-hidden="true" title="Reading the console"></span>
    {:else if modulesStore.checking}
      <span class="muted small checking"><span class="spinner" aria-hidden="true"></span> Checking for updates</span>
    {/if}
  </div>
  <p class="muted small">RetroArch and its cores run the systems the console has no emulator for. Installing, updating or removing one restarts the console once.</p>
  {#if modulesStore.status}
    <ul class="modules">
      {#each modulesStore.status.modules as module (module.id)}
        {@const progress = modulesStore.progress[module.id]}
        {@const downloading = modulesStore.downloading.includes(module.id)}
        <li class:wide={module.id === "retroarch"}>
          <div class="module-text">
            <strong>{module.short}</strong>
            <span class="muted small">
              {module.purpose}
              {#if module.warning}
                <span class="risk" title={module.warning}><Icon name="warning" size={12} /> At your own risk</span>
              {/if}
              ·
              {#if downloading && progress}
                Downloading {formatBytes(progress.done)}{progress.total ? ` of ${formatBytes(progress.total)}` : ""}
              {:else if module.installed === true && module.update === true}
                <span class="update-text">Update available</span>
              {:else if module.installed === true}
                <span class="ok-text">Installed</span>
              {:else}
                {state(module)}
              {/if}
            </span>
          </div>
          {#if module.installed === true && module.update === true}
            <button
              type="button"
              class="btn btn-sm btn-primary"
              onclick={() => void modulesStore.update(module.id)}
              disabled={flowStore.running || !ready}
              title="Replaces it with the Mod Hub's current build and restarts the console once. Games keep working."
            >
              Update
            </button>
          {/if}
          {#if module.installed === true}
            <button
              type="button"
              class="btn btn-sm"
              onclick={() => modulesStore.requestRemove(module.id)}
              disabled={flowStore.running || !ready}
              title={module.id === "retroarch" ? "Uninstalls RetroArch and its cores, then reboots the console once" : "Uninstalls the core and reboots the console once"}
            >
              Remove
            </button>
          {:else}
            <button
              type="button"
              class="btn btn-sm"
              onclick={() => void modulesStore.install(modulesStore.missing(withRetroarch(module.id)))}
              disabled={downloading || flowStore.running || !ready}
              title={ready ? "Downloads the module and reboots the console once" : "Connect a console running the custom system"}
            >
              {#if downloading}<span class="spinner" aria-hidden="true"></span>{/if}
              Install
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted"><span class="spinner" aria-hidden="true"></span> Reading the modules…</p>
  {/if}
  {#if !ready}
    <p class="muted small foot">Connect the console running the custom system to install or remove emulators.</p>
  {/if}
</section>

<style>
  .emulators {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-5);
    min-height: 0;
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  /* RetroArch across the top, then the cores two to a row. */
  .modules {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-2) var(--space-5);
  }

  .modules li.wide {
    grid-column: 1 / -1;
  }

  .modules li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .risk {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: var(--space-1);
    color: var(--warn);
  }

  .module-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .modules .btn {
    flex: none;
    min-width: 76px;
    justify-content: center;
  }

  .small {
    font-size: var(--text-sm);
  }

  .ok-text {
    color: var(--ok);
  }

  .update-text {
    color: var(--accent-strong);
    font-weight: var(--weight-semibold);
  }

  .checking {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .foot {
    margin-top: auto;
    padding-top: var(--space-2);
  }

  @media (max-width: 799px) {
    .modules {
      grid-template-columns: 1fr;
    }
  }
</style>
