<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { inTauri } from "../../api/tauri";
  import { consoleStore, flowStore, formatBytes, packageStore, toasts } from "../../state";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import { consoleWatch } from "./watch";

  /* The kernel tasks besides the first install. With hakchi installed: reinstall or update
     it, back up the stock kernel, factory reset. Without: boot from memory, back up, reset.
     Each task's progress opens in the progress window. */
  interface Props {
    /** The console runs its installed hakchi. */
    installed: boolean;
  }

  let { installed }: Props = $props();

  const watch = $derived(consoleWatch(consoleStore.report));
  const pkg = $derived(packageStore.status?.package ?? null);
  const busy = $derived(flowStore.running || flowStore.starting);
  /** Kernel flows boot hakchi, so they need the console reachable and the package. */
  const canStart = $derived(watch.ready && pkg !== null && !busy);
  const reason = $derived(
    busy ? "Another task is running on the console." : !watch.ready ? "Connect the console first." : !pkg ? "Download the kernel package first." : "",
  );
  let confirmReset = $state(false);

  async function dumpKernel(): Promise<void> {
    if (!inTauri) {
      toasts.info("The save dialog needs the desktop app.");
      return;
    }
    const path = await save({
      title: "Save the stock kernel",
      defaultPath: "kernel.img",
      filters: [{ name: "Kernel image", extensions: ["img"] }],
    });
    if (path) {
      await flowStore.begin({ kind: "dump_kernel", path });
    }
  }

  function reset(): void {
    confirmReset = false;
    void flowStore.begin({ kind: "uninstall", stock_kernel: null, force_stock_kernel: false });
  }
</script>

<section class="card maintenance" aria-labelledby="maintenance-heading">
  <h2 id="maintenance-heading">hakchi</h2>
  {#if installed}
    <div class="package">
      <span class="muted small">
        {#if pkg}
          Package <span class="num">{pkg.version}</span>, {formatBytes(pkg.len)}
        {:else if packageStore.updating}
          Downloading the package…
        {:else}
          No kernel package downloaded.
        {/if}
      </span>
      <button type="button" class="btn btn-quiet btn-sm" onclick={() => void packageStore.update()} disabled={busy || packageStore.updating}>
        {#if packageStore.updating}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="download" size={14} />{/if}
        {pkg ? "Update" : "Download"}
      </button>
    </div>
  {/if}
  <ul class="tools" title={canStart ? undefined : reason}>
    {#if installed}
      <li>
        <div class="tool-text">
          <strong>Reinstall or repair</strong>
          <span class="muted">Writes the package's hakchi again. Games and emulators stay.</span>
        </div>
        <button type="button" class="btn btn-sm" disabled={!canStart} onclick={() => void flowStore.begin({ kind: "install" })}>
          <Icon name="refresh" size={14} />
          Reinstall
        </button>
      </li>
    {:else}
      <li>
        <div class="tool-text">
          <strong>Boot from memory</strong>
          <span class="muted">Runs hakchi once. Nothing is written.</span>
        </div>
        <button type="button" class="btn btn-sm" disabled={!canStart} onclick={() => void flowStore.begin({ kind: "memboot" })}>
          <Icon name="play" size={14} />
          Boot
        </button>
      </li>
    {/if}
    <li>
      <div class="tool-text">
        <strong>Save the stock kernel</strong>
        <span class="muted">A backup file, to restore the console if its own copy is lost.</span>
      </div>
      <button type="button" class="btn btn-sm" disabled={!canStart} onclick={() => void dumpKernel()}>
        <Icon name="save" size={14} />
        Save…
      </button>
    </li>
    <li>
      <div class="tool-text">
        <strong>Factory reset</strong>
        <span class="muted">Removes hakchi and restores the console's own system.</span>
      </div>
      <button type="button" class="btn btn-sm" disabled={!canStart} onclick={() => (confirmReset = true)}>
        <Icon name="undo" size={14} />
        Reset…
      </button>
    </li>
  </ul>
  {#if !canStart && reason}
    <p class="muted small">{reason}</p>
  {/if}
</section>

<Dialog open={confirmReset} title="Factory reset" width="520px" onclose={() => (confirmReset = false)}>
  <p>
    This flashes the original kernel back and removes hakchi from the console, with the games and
    emulators added through it. The console's own games are untouched.
  </p>
  {#snippet actions()}
    <button type="button" class="btn btn-danger" onclick={reset}>Remove hakchi and restore the console</button>
    <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (confirmReset = false)}>Keep hakchi</button>
  {/snippet}
</Dialog>

<style>
  .maintenance {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-5);
  }

  .package {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    justify-content: space-between;
  }

  .tools {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
  }

  .tools li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
    border-top: 1px solid var(--border);
  }

  .tool-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--text-md);
  }

  .tool-text .muted {
    font-size: var(--text-sm);
  }

  .tools .btn {
    flex: none;
    min-width: 96px;
    justify-content: center;
  }

  .small {
    font-size: var(--text-sm);
  }
</style>
