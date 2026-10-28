<script lang="ts">
  import { consoleStore, flowStore, formatBytes, packageStore } from "../../state";
  import Icon from "../Icon.svelte";
  import FelInstructions from "./FelInstructions.svelte";
  import { consoleWatch } from "./watch";

  /* Installing hakchi on a console that does not have it, in one card: how to put the
     console in FEL mode with what the app sees right now, then the kernel package and the
     install button side by side. The package is built into the app; the download is only a
     fallback for when it cannot be written out. The install's progress opens in the progress
     window. */
  const watch = $derived(consoleWatch(consoleStore.report));
  const pkg = $derived(packageStore.status?.package ?? null);
  const busy = $derived(flowStore.running || flowStore.starting);
  const canStart = $derived(watch.ready && pkg !== null && !busy);

  /** What the install waits for, in one short line. */
  const blocker = $derived.by((): string => {
    if (flowStore.running) {
      return "Another task is running on the console.";
    }
    if (!pkg) {
      return "The kernel package is not ready.";
    }
    if (!watch.ready) {
      return "Waiting for the console in FEL mode.";
    }
    return "About five minutes; the console reboots twice.";
  });
</script>

<section class="card setup" aria-labelledby="setup-heading">
  <h2 id="setup-heading">Install hakchi</h2>

  <div class="fel">
    <FelInstructions width={190} />
    <p class="watch" role="status">
      <span class="dot tone-{watch.tone}" aria-hidden="true"></span>
      {watch.text}
    </p>
  </div>

  <div class="footer">
    <div class="package">
      {#if packageStore.loading && !packageStore.status}
        <span class="muted"><span class="spinner" aria-hidden="true"></span> Preparing the kernel package…</span>
      {:else if packageStore.updating}
        <span class="muted num">
          <span class="spinner" aria-hidden="true"></span>
          Downloading the kernel package{#if packageStore.progress?.total}: {formatBytes(packageStore.progress.done)} of {formatBytes(packageStore.progress.total)}{/if}
        </span>
      {:else if pkg}
        <span class="ok"><Icon name="check" size={16} /></span>
        <span>Kernel package <strong class="num">{pkg.version}</strong> <span class="muted num">· {formatBytes(pkg.len)}{pkg.source === "override" ? " (CARTLOAD_HMOD)" : ""}</span></span>
      {:else}
        <div class="missing">
          <span>The kernel package is missing. Download it (9 MB).</span>
          {#if packageStore.status?.problem}
            <span class="problem">The copy on disk cannot be used: {packageStore.status.problem}</span>
          {/if}
          {#if packageStore.error}
            <span class="problem">The download failed: {packageStore.error}</span>
          {/if}
        </div>
        <button type="button" class="btn" onclick={() => void packageStore.update()} disabled={busy}>
          <Icon name="download" size={16} />
          {packageStore.error ? "Try again" : "Download"}
        </button>
      {/if}
    </div>

    <div class="install">
      <button type="button" class="btn btn-primary" onclick={() => void flowStore.begin({ kind: "install" })} disabled={!canStart}>
        {#if flowStore.starting}
          <span class="spinner on-accent" aria-hidden="true"></span>
          Starting…
        {:else}
          Install hakchi
          <Icon name="arrow" size={16} />
        {/if}
      </button>
      <span class="muted small">{blocker}</span>
    </div>
  </div>
</section>

<style>
  .setup {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-5);
  }

  .fel {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .watch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control);
    margin: 0;
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--surface-raised);
  }

  /* The package and the install button share the card's last line, pushed to the bottom
     when the card stretches to the column's height. */
  .footer {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
  }

  .package {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .missing {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .ok {
    display: inline-flex;
    color: var(--ok);
  }

  .install {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }

  .small {
    font-size: var(--text-sm);
  }

  .problem {
    color: var(--danger);
    font-size: var(--text-sm);
  }

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }

  @media (max-width: 799px) {
    .footer {
      flex-direction: column;
      align-items: stretch;
    }

    .install {
      align-items: flex-start;
    }
  }
</style>
