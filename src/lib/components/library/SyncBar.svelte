<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import { inTauri } from "../../api/tauri";
  import { previewDialog } from "../../dev/preview";
  import { NORMAL, ms } from "../../motion";
  import { consoleStore, formatBytes, formatDuration, libraryStore, preferences, syncStore } from "../../state";

  const status = $derived(consoleStore.report?.status ?? null);
  const connected = $derived(status?.state === "connected");
  const storage = $derived(
    syncStore.plan?.storage ?? (status?.state === "connected" && status.info.storage ? status.info.storage.usage : null),
  );
  const plan = $derived(syncStore.plan);
  const snapshot = $derived(syncStore.snapshot);
  const running = $derived(syncStore.running);
  const libSize = $derived(libraryStore.view?.total_size ?? 0);

  /** The bar's one sentence about pending work. */
  const pending = $derived.by((): string => {
    if (running) {
      return "Syncing…";
    }
    if (!connected) {
      return status?.state === "working" ? "The console is busy." : "Connect the console to compare.";
    }
    if (syncStore.planning && !plan) {
      return "Comparing with the console…";
    }
    if (syncStore.planError) {
      return syncStore.planError;
    }
    if (!plan) {
      return "Comparing with the console…";
    }
    if (!plan.fits) {
      return `Does not fit: ${formatBytes(plan.total_size)} needed, ${formatBytes(plan.available ?? 0)} available.`;
    }
    if (plan.noop) {
      return "The console is up to date.";
    }
    // Explains the N of "Sync N changes": every change is one of these four.
    const parts: string[] = [];
    if (plan.uploads.length > 0) {
      parts.push(`${plan.uploads.length} file${plan.uploads.length === 1 ? "" : "s"} to upload (${formatBytes(plan.upload_size)})`);
    }
    if (plan.deletions.length > 0) {
      parts.push(`${plan.deletions.length} file${plan.deletions.length === 1 ? "" : "s"} to remove`);
    }
    if (plan.link_deletions.length > 0) {
      parts.push(`${plan.link_deletions.length} link${plan.link_deletions.length === 1 ? "" : "s"} to reset`);
    }
    if (plan.empty_dirs.length > 0) {
      parts.push(`${plan.empty_dirs.length} empty folder${plan.empty_dirs.length === 1 ? "" : "s"} to remove`);
    }
    return `${plan.changes} change${plan.changes === 1 ? "" : "s"}: ${parts.join(", ")}.`;
  });

  /** Files on the console that this library does not have: typically hakchi2 CE's copies. */
  const replacesOthers = $derived(plan !== null && plan.deletions.length + plan.link_deletions.length > 0);
  let changesOpen = $state(false);

  onMount(() => {
    if (!inTauri && previewDialog() === "changes") {
      changesOpen = true;
    }
  });

  const canSync = $derived(connected && plan !== null && plan.fits && !running && !syncStore.starting && !libraryStore.importing);
  const syncLabel = $derived(plan && !plan.noop ? `Sync ${plan.changes} change${plan.changes === 1 ? "" : "s"}` : "Sync");
  const stepPercent = $derived.by((): number => {
    const step = snapshot.step;
    if (!step) {
      return 0;
    }
    const upload = snapshot.upload;
    const within = upload && upload.overall_total > 0 ? upload.overall_done / upload.overall_total : 0;
    return Math.min(100, ((step.index - 1 + within) / step.total) * 100);
  });
  const elapsed = $derived(
    snapshot.started_at === null ? "" : formatDuration((snapshot.finished_at ?? syncStore.now) - snapshot.started_at),
  );
  const consolePercent = $derived(storage && storage.total > 0 ? Math.min(100, (storage.used / storage.total) * 100) : 0);
  const plannedPercent = $derived(
    storage && storage.total > 0 && plan ? Math.min(100, ((storage.used + plan.upload_size) / storage.total) * 100) : consolePercent,
  );
</script>

{#snippet changeLists(p: NonNullable<typeof plan>, expanded: boolean)}
  <div class="columns">
    <details open={expanded || (p.uploads.length > 0 && p.uploads.length <= 12)}>
      <summary><Icon name="upload" size={14} /> Upload {p.uploads.length} file{p.uploads.length === 1 ? "" : "s"} ({formatBytes(p.upload_size)})</summary>
      <ul class="files mono">
        {#each p.uploads as file (file.path)}
          <li><span>{file.path.replace(/^\.\//, "")}</span><span class="num">{formatBytes(file.size)}</span></li>
        {:else}
          <li class="muted">Nothing to upload.</li>
        {/each}
      </ul>
    </details>
    <details open={expanded || (p.deletions.length + p.link_deletions.length + p.empty_dirs.length > 0 && p.deletions.length <= 12)}>
      <summary><Icon name="trash" size={14} /> Remove {p.deletions.length} file{p.deletions.length === 1 ? "" : "s"}, reset {p.link_deletions.length} link{p.link_deletions.length === 1 ? "" : "s"}, remove {p.empty_dirs.length} empty folder{p.empty_dirs.length === 1 ? "" : "s"}</summary>
      <ul class="files mono">
        {#each p.deletions as path (path)}
          <li><span>{path.replace(/^\.\//, "")}</span><span class="muted">file</span></li>
        {/each}
        {#each p.link_deletions as path (path)}
          <li><span>{path.replace(/^\.\//, "")}</span><span class="muted">link</span></li>
        {/each}
        {#each p.empty_dirs as path (path)}
          <li><span>{path.replace(/^\.\//, "")}</span><span class="muted">folder</span></li>
        {/each}
        {#if p.deletions.length + p.link_deletions.length + p.empty_dirs.length === 0}
          <li class="muted">Nothing to remove.</li>
        {/if}
      </ul>
    </details>
  </div>
  {#if p.deletions.length + p.link_deletions.length > 0}
    <p class="muted">If hakchi2 CE synced this console, the first Cartload sync replaces its files with this library.</p>
  {/if}
{/snippet}

<footer class="sync card" aria-label="Sync">
  {#if syncStore.reviewing && plan}
    <section class="panel" aria-label="Sync plan" transition:slide={{ duration: ms(NORMAL) }}>
      <div class="panel-head">
        <h3>What the sync will do</h3>
        <span class="muted num">{plan.games} games, {plan.originals} built in, {plan.folders} folders · {formatBytes(plan.total_size)} on the console</span>
      </div>
      {#if plan.noop}
        <p>Nothing to transfer: the console already holds every file. Syncing anyway re-links the games and restarts the menu.</p>
      {/if}
      {@render changeLists(plan, false)}
      <p class="muted">{plan.unchanged} file{plan.unchanged === 1 ? "" : "s"} already on the console stay as they are. The console's menu restarts when the sync ends.</p>
      <div class="panel-actions">
        <button type="button" class="btn btn-primary" onclick={() => syncStore.run()} disabled={!canSync}>
          {#if syncStore.starting}<span class="spinner on-accent" aria-hidden="true"></span>{:else}<Icon name="sync" size={16} />{/if}
          <span>{syncStore.starting ? "Starting…" : "Sync now"}</span>
        </button>
        <button type="button" class="btn btn-quiet" onclick={() => syncStore.cancelReview()}>Cancel</button>
      </div>
    </section>
  {:else if running || snapshot.state === "done" || snapshot.state === "failed"}
    <section class="panel progress" aria-live="polite" aria-label="Sync progress" aria-busy={running} transition:slide={{ duration: ms(NORMAL) }}>
      <div class="panel-head">
        {#if running}
          <h3>Syncing games{snapshot.step ? `: step ${snapshot.step.index} of ${snapshot.step.total}, ${snapshot.step.label.toLowerCase()}` : "…"}</h3>
        {:else if snapshot.state === "done" && snapshot.report}
          <h3 class="ok"><Icon name="check" size={16} /> Synced in {elapsed}</h3>
        {:else}
          <h3 class="failed"><Icon name="warning" size={16} /> The sync failed</h3>
        {/if}
        <span class="muted num">{elapsed}</span>
      </div>
      {#if running}
        <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(stepPercent)}>
          <span style:width="{stepPercent}%"></span>
        </div>
        {#if snapshot.upload}
          <p class="muted num">{snapshot.upload.path.replace(/^\.\//, "")} · {formatBytes(snapshot.upload.overall_done)} of {formatBytes(snapshot.upload.overall_total)}</p>
        {:else if snapshot.step?.detail}
          <p class="muted">{snapshot.step.detail}</p>
        {/if}
        <p class="muted">Leave the console and the cable alone until it finishes.</p>
      {:else if snapshot.state === "done" && snapshot.report}
        <p>
          {snapshot.report.uploaded_files} file{snapshot.report.uploaded_files === 1 ? "" : "s"} uploaded ({formatBytes(snapshot.report.uploaded_bytes)}),
          {snapshot.report.removed_files} removed{snapshot.report.games_mounted ? ". The console's menu is back with the games." : ". The console reported no games after mounting; check its menu."}
        </p>
      {:else if snapshot.error}
        <p>{snapshot.error.message}</p>
        {#if snapshot.error.detail}
          <details><summary>Details</summary><pre>{snapshot.error.detail}</pre></details>
        {/if}
      {/if}
      {#if snapshot.log.length > 0}
        <p class="log mono muted">{snapshot.log[snapshot.log.length - 1]}</p>
      {/if}
      <div class="panel-actions">
        {#if !running}
          <button type="button" class="btn" onclick={() => syncStore.dismiss()}>Done</button>
          {#if snapshot.state === "failed"}
            <button type="button" class="btn btn-quiet" onclick={() => syncStore.review()}>Try again</button>
          {/if}
        {/if}
        <button type="button" class="btn btn-quiet" onclick={() => preferences.setLogOpen(true)}>Full log</button>
      </div>
    </section>
  {/if}

  <div class="bar-row">
    <div class="block">
      <span class="caps">Console</span>
      {#if storage}
        <div class="meter" title="Used, planned, total">
          <span class="planned" style:width="{plannedPercent}%"></span>
          <span class="used" style:width="{consolePercent}%"></span>
        </div>
        <span class="num muted">{formatBytes(storage.used)} of {formatBytes(storage.total)} used{plan && plan.upload_size > 0 ? `, +${formatBytes(plan.upload_size)}` : ""}</span>
      {:else}
        <span class="muted">{connected ? "Reading…" : "Not connected"}</span>
      {/if}
    </div>
    <div class="block">
      <span class="caps">This library</span>
      <span class="num muted">{formatBytes(libSize)} · {libraryStore.gameCount} game{libraryStore.gameCount === 1 ? "" : "s"}</span>
    </div>
    <div class="block pending" class:warn={plan !== null && !plan.fits}>
      <span class="caps">Changes</span>
      <span class="muted pending-text">
        {#if syncStore.planning && !plan}<span class="spinner spinner-sm" aria-hidden="true"></span>{/if}
        <span>{pending}</span>
        {#if plan && !plan.noop && plan.fits && !running}
          <button type="button" class="btn btn-quiet btn-sm" onclick={() => (changesOpen = true)}>Show changes</button>
        {/if}
      </span>
      {#if replacesOthers && !running}
        <span class="muted note">If hakchi2 CE synced this console, the first Cartload sync replaces its files with this library.</span>
      {/if}
    </div>
    <button type="button" class="btn btn-primary" onclick={() => syncStore.review()} disabled={!canSync || syncStore.reviewing}>
      <Icon name="sync" size={16} />
      <span>{syncStore.starting ? "Starting…" : syncLabel}</span>
    </button>
  </div>
</footer>

{#if plan}
  <Dialog open={changesOpen} title="What the sync will change" width="760px" onclose={() => (changesOpen = false)}>
    <p class="muted num">{plan.games} games, {plan.originals} built in, {plan.folders} folders · {formatBytes(plan.total_size)} on the console · {plan.unchanged} file{plan.unchanged === 1 ? "" : "s"} unchanged</p>
    {@render changeLists(plan, true)}
    {#snippet actions()}
      <button type="button" class="btn btn-primary" onclick={() => { changesOpen = false; syncStore.review(); }} disabled={!canSync || syncStore.reviewing}>
        <Icon name="sync" size={16} />
        <span>{syncLabel}</span>
      </button>
      <button type="button" class="btn btn-quiet" onclick={() => (changesOpen = false)}>Close</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .sync {
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-m);
  }

  /* The explanation of the changes takes the rest of the row; on a narrow window it wraps
     onto its own line rather than stacking its words. */
  .bar-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-5);
    padding: var(--space-2) var(--space-4);
    font-size: var(--text-md);
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .block .caps {
    font-size: var(--text-xs);
  }

  .pending {
    flex: 1 1 420px;
  }

  .bar-row > .btn-primary {
    margin-left: auto;
  }

  .pending-text {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-2);
  }

  .note {
    font-size: var(--text-sm);
  }

  .pending.warn .pending-text {
    color: var(--warn);
  }

  .meter {
    position: relative;
    width: 192px;
    height: 6px;
    border-radius: 999px;
    background: var(--border);
    overflow: hidden;
  }

  .meter span {
    position: absolute;
    left: 0;
    top: 0;
    height: 100%;
    border-radius: 999px;
    transition: width var(--motion) var(--ease-out);
  }

  .meter .used {
    background: var(--accent);
  }

  .meter .planned {
    background: var(--accent-soft);
    border-right: 1px solid var(--accent);
  }

  .panel {
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: var(--text-md);
    max-height: 40vh;
    overflow: auto;
  }

  .panel-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .panel h3 {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }

  .ok {
    color: var(--ok);
  }

  .failed {
    color: var(--danger);
  }

  .columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
  }

  .columns summary {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .files {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--text-sm);
  }

  /* In the bar's review panel the lists stay short; the dialog lets its content scroll. */
  .panel .files {
    max-height: 144px;
    overflow: auto;
  }

  .files li {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
    padding: 1px 0;
  }

  .panel-actions {
    display: flex;
    gap: var(--space-2);
  }

  .bar {
    height: 8px;
    border-radius: 999px;
    background: var(--border);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width var(--motion-fast) linear;
  }

  .log {
    font-size: var(--text-sm);
    white-space: pre-wrap;
    word-break: break-word;
  }

  pre {
    margin: var(--space-1) 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }
</style>
