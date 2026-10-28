<script lang="ts">
  import { consoleStore, flowStore, formatBytes, packageStore } from "../../state";
  import Icon from "../Icon.svelte";
  import FelInstructions from "./FelInstructions.svelte";
  import { consoleWatch } from "./watch";

  /* Installing hakchi on a console that does not have it: FEL mode, the kernel package,
     then the install, whose progress opens in the progress window. */
  const watch = $derived(consoleWatch(consoleStore.report));
  const pkg = $derived(packageStore.status?.package ?? null);
  const busy = $derived(flowStore.running || flowStore.starting);
  const canStart = $derived(watch.ready && pkg !== null && !busy);

  /** Why the install cannot start, in one short line. */
  const blocker = $derived.by((): string | null => {
    if (busy) {
      return flowStore.running ? "Another task is running on the console." : null;
    }
    if (!watch.ready) {
      return "Waiting for the console in FEL mode (step 1).";
    }
    if (!pkg) {
      return "Download the kernel package (step 2) first.";
    }
    return null;
  });
</script>

<ol class="stages">
  <li class="stage" class:complete={watch.ready}>
    <span class="number" aria-hidden="true">{#if watch.ready}<Icon name="check" size={16} />{:else}1{/if}</span>
    <div class="body card">
      <h2>Put the console in FEL mode</h2>
      <FelInstructions />
      <p class="watch" role="status">
        <span class="dot tone-{watch.tone}" aria-hidden="true"></span>
        {watch.text}
      </p>
    </div>
  </li>

  <li class="stage" class:complete={pkg !== null}>
    <span class="number" aria-hidden="true">{#if pkg}<Icon name="check" size={16} />{:else}2{/if}</span>
    <div class="body card">
      <div class="row">
        <h2>Kernel package</h2>
        <span class="spacer"></span>
        {#if packageStore.loading && !packageStore.status}
          <span class="muted"><span class="spinner" aria-hidden="true"></span> Checking the cache…</span>
        {:else if packageStore.updating}
          <span class="muted num">
            {#if packageStore.progress?.total}
              {formatBytes(packageStore.progress.done)} of {formatBytes(packageStore.progress.total)}
            {:else}
              Downloading…
            {/if}
          </span>
        {:else if pkg}
          <button type="button" class="btn btn-quiet" onclick={() => void packageStore.update()} disabled={busy}>
            <Icon name="download" size={16} />
            Update package
          </button>
        {/if}
      </div>
      {#if pkg}
        <p>
          hakchi <strong class="num">{pkg.version}</strong>
          <span class="muted num">· {formatBytes(pkg.len)}{pkg.source === "override" ? " (from CARTLOAD_HMOD)" : ""}</span>
        </p>
        {#if packageStore.error}
          <p class="problem">The update failed: {packageStore.error} The copy on disk is still usable.</p>
        {/if}
      {:else if packageStore.updating}
        <progress max={packageStore.progress?.total ?? 1} value={packageStore.progress?.total ? packageStore.progress.done : undefined}></progress>
      {:else}
        <p>No kernel package yet. It is a 9 MB download from the hakchi project's servers.</p>
        {#if packageStore.status?.problem}
          <p class="problem">The copy on disk cannot be used: {packageStore.status.problem}</p>
        {/if}
        {#if packageStore.error}
          <p class="problem">The download failed: {packageStore.error}</p>
        {/if}
        <div>
          <button type="button" class="btn btn-primary" onclick={() => void packageStore.update()} disabled={busy}>
            <Icon name="download" size={16} />
            {packageStore.error ? "Try the download again" : "Download package"}
          </button>
        </div>
      {/if}
    </div>
  </li>

  <li class="stage" class:active={canStart}>
    <span class="number" aria-hidden="true">3</span>
    <div class="body card">
      <div class="row">
        <h2>Install hakchi</h2>
        <span class="spacer"></span>
        <button type="button" class="btn btn-primary" onclick={() => void flowStore.begin({ kind: "install" })} disabled={!canStart}>
          {#if flowStore.starting}
            <span class="spinner on-accent" aria-hidden="true"></span>
            Starting…
          {:else}
            Install hakchi
            <Icon name="arrow" size={16} />
          {/if}
        </button>
      </div>
      <p class="muted small">
        {blocker ?? "Takes about five minutes. The console reboots twice; the progress opens in a window."}
      </p>
    </div>
  </li>
</ol>

<style>
  .stages {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .stage {
    position: relative;
    display: grid;
    grid-template-columns: 36px 1fr;
    gap: var(--space-4);
  }

  .stage::before {
    content: "";
    position: absolute;
    left: 17px;
    top: 40px;
    bottom: calc(-1 * var(--space-4));
    width: 2px;
    background: var(--border);
  }

  .stage:last-child::before {
    display: none;
  }

  .number {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-weight: var(--weight-bold);
    font-variant-numeric: tabular-nums;
    transition:
      background-color var(--motion) var(--ease-out),
      border-color var(--motion) var(--ease-out),
      color var(--motion) var(--ease-out);
  }

  .stage.complete .number {
    background: var(--ok-soft);
    border-color: var(--ok);
    color: var(--ok);
  }

  .stage.active .number {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent-strong);
  }

  .body {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--control);
  }

  .spacer {
    flex: 1;
  }

  .watch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control);
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--surface-raised);
  }

  .small {
    font-size: var(--text-md);
  }

  .problem {
    color: var(--danger);
    font-size: var(--text-md);
  }

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }
</style>
