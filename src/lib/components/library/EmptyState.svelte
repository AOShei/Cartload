<script lang="ts">
  import Cartridge from "../Cartridge.svelte";
  import Icon from "../Icon.svelte";
  import { consoleStore, libraryStore, navigation } from "../../state";

  const status = $derived(consoleStore.report?.status ?? null);
  const connected = $derived(status?.state === "connected");
  const installed = $derived(connected && !(status?.state === "connected" && status.info.minimal_memboot));
  const fel = $derived(status?.state === "fel");
</script>

<section class="empty card" aria-labelledby="empty-heading">
  <div class="glyph"><Cartridge size={64} /></div>
  <h1 id="empty-heading">Three steps to games on the console</h1>
  <ol class="steps">
    <li class:done={connected}>
      <span class="mark">{#if connected}<Icon name="check" size={16} />{:else}1{/if}</span>
      <div>
        <strong>Connect the console</strong>
        <p class="muted">
          {#if connected}
            Connected: {consoleStore.report?.title}
          {:else if fel}
            The console is in FEL mode, ready for the kernel.
          {:else}
            Plug it in with the USB cable and turn it on. The status bar at the bottom shows what is found.
          {/if}
        </p>
      </div>
    </li>
    <li class:done={installed}>
      <span class="mark">{#if installed}<Icon name="check" size={16} />{:else}2{/if}</span>
      <div>
        <strong>Install the custom system</strong>
        <p class="muted">
          {#if installed}
            The custom system is running on the console.
          {:else}
            The Console page walks through it once; the console keeps it afterwards.
          {/if}
        </p>
        {#if !installed}
          <button type="button" class="btn" onclick={() => navigation.go("console")}>Open the Console page</button>
        {/if}
      </div>
    </li>
    <li>
      <span class="mark">3</span>
      <div>
        <strong>Add games</strong>
        <p class="muted">Drop ROM files or folders anywhere in this window, or choose them. SNES (.sfc, .smc) and NES (.nes) ROMs, or zips of them.</p>
        <div class="actions">
          <button type="button" class="btn btn-primary" onclick={() => libraryStore.addGames()} disabled={libraryStore.importing}>
            <Icon name="plus" size={16} />
            <span>Add games…</span>
          </button>
          <button type="button" class="btn btn-quiet" onclick={() => libraryStore.addFolder()} disabled={libraryStore.importing}>Add a folder of ROMs…</button>
        </div>
      </div>
    </li>
  </ol>
</section>

<style>
  .empty {
    max-width: 704px;
    margin: var(--space-5) auto;
    padding: var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .glyph {
    color: var(--accent);
    display: flex;
  }

  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .steps li {
    display: flex;
    gap: var(--space-3);
  }

  .steps li > div {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: flex-start;
  }

  .mark {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-raised);
    border: 1px solid var(--border-strong);
    font-weight: var(--weight-semibold);
    font-size: var(--text-md);
    transition:
      background-color var(--motion) var(--ease-out),
      border-color var(--motion) var(--ease-out),
      color var(--motion) var(--ease-out);
  }

  .done .mark {
    background: var(--ok-soft);
    border-color: var(--ok);
    color: var(--ok);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
  }
</style>
