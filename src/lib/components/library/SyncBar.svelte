<script lang="ts">
  import Icon from "../Icon.svelte";
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
    const parts: string[] = [];
    if (plan.uploads.length > 0) {
      parts.push(`${plan.uploads.length} file${plan.uploads.length === 1 ? "" : "s"} to upload (${formatBytes(plan.upload_size)})`);
    }
    const removals = plan.deletions.length + plan.link_deletions.length + plan.empty_dirs.length;
    if (removals > 0) {
      parts.push(`${removals} to remove`);
    }
    return parts.join(", ") + ".";
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

<footer class="sync card" aria-label="Sync">
  {#if syncStore.reviewing && plan}
    <section class="panel" aria-label="Sync plan">
      <div class="panel-head">
        <h3>What the sync will do</h3>
        <span class="muted num">{plan.games} games, {plan.originals} built in, {plan.folders} folders · {formatBytes(plan.total_size)} on the console</span>
      </div>
      {#if plan.noop}
        <p>Nothing to transfer: the console already holds every file. Syncing anyway re-links the games and restarts the menu.</p>
      {/if}
      <div class="columns">
        <details open={plan.uploads.length > 0 && plan.uploads.length <= 12}>
          <summary><Icon name="upload" size={14} /> Upload {plan.uploads.length} file{plan.uploads.length === 1 ? "" : "s"} ({formatBytes(plan.upload_size)})</summary>
          <ul class="files mono">
            {#each plan.uploads as file (file.path)}
              <li><span>{file.path.replace(/^\.\//, "")}</span><span class="num">{formatBytes(file.size)}</span></li>
            {/each}
          </ul>
        </details>
        <details open={plan.deletions.length + plan.link_deletions.length + plan.empty_dirs.length > 0 && plan.deletions.length <= 12}>
          <summary><Icon name="trash" size={14} /> Remove {plan.deletions.length} file{plan.deletions.length === 1 ? "" : "s"}, {plan.link_deletions.length} link{plan.link_deletions.length === 1 ? "" : "s"}, {plan.empty_dirs.length} director{plan.empty_dirs.length === 1 ? "y" : "ies"}</summary>
          <ul class="files mono">
            {#each [...plan.deletions, ...plan.link_deletions, ...plan.empty_dirs] as path (path)}
              <li><span>{path.replace(/^\.\//, "")}</span></li>
            {/each}
          </ul>
        </details>
      </div>
      <p class="muted">{plan.unchanged} file{plan.unchanged === 1 ? "" : "s"} already on the console stay as they are. The console's menu restarts when the sync ends.</p>
      <div class="panel-actions">
        <button type="button" class="btn btn-primary" onclick={() => syncStore.run()} disabled={!canSync}>
          <Icon name="sync" size={16} />
          <span>Sync now</span>
        </button>
        <button type="button" class="btn btn-quiet" onclick={() => syncStore.cancelReview()}>Cancel</button>
      </div>
    </section>
  {:else if running || snapshot.state === "done" || snapshot.state === "failed"}
    <section class="panel progress" aria-live="polite" aria-label="Sync progress">
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
      <span class="label">Console</span>
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
      <span class="label">This library</span>
      <span class="num muted">{formatBytes(libSize)} · {libraryStore.gameCount} game{libraryStore.gameCount === 1 ? "" : "s"}</span>
    </div>
    <div class="block pending" class:warn={plan !== null && !plan.fits}>
      <span class="label">Changes</span>
      <span class="muted">{pending}</span>
    </div>
    <button type="button" class="btn btn-primary" onclick={() => syncStore.review()} disabled={!canSync || syncStore.reviewing}>
      <Icon name="sync" size={16} />
      <span>{syncStore.starting ? "Starting…" : syncLabel}</span>
    </button>
  </div>
</footer>

<style>
  .sync {
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-m);
  }

  .bar-row {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-2) var(--space-4);
    font-size: 0.85rem;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .pending {
    flex: 1;
  }

  .pending.warn span:last-child {
    color: var(--warn);
  }

  .meter {
    position: relative;
    width: 12rem;
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
    font-size: 0.9rem;
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
    margin: 0;
    font-size: 1rem;
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
    max-height: 9rem;
    overflow: auto;
    font-size: 0.75rem;
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
    transition: width 150ms linear;
  }

  .log {
    font-size: 0.75rem;
    white-space: pre-wrap;
    word-break: break-word;
  }

  pre {
    margin: var(--space-1) 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
</style>
