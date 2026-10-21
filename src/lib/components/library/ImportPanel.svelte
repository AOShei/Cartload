<script lang="ts">
  import Icon from "../Icon.svelte";
  import { baseName, libraryStore } from "../../state";

  const files = $derived(libraryStore.importFiles);
  const report = $derived(libraryStore.importReport);
  const done = $derived(files.filter((f) => f.outcome !== null).length);
  const total = $derived(files[0]?.total ?? files.length);

  const heading = $derived.by((): string => {
    if (libraryStore.importing) {
      return `Adding games: ${done} of ${total}…`;
    }
    if (!report) {
      return "Adding games";
    }
    const parts = [`${report.imported} game${report.imported === 1 ? "" : "s"} added`];
    if (report.unmatched > 0) {
      parts.push(`${report.unmatched} not in the hash database`);
    }
    if (report.failed > 0) {
      parts.push(`${report.failed} failed`);
    }
    if (report.skipped > 0) {
      parts.push(`${report.skipped} skipped`);
    }
    return parts.join(", ") + ".";
  });
</script>

<section class="import" aria-live="polite" aria-label="Import results">
  <div class="head">
    <h3>{heading}</h3>
    {#if !libraryStore.importing}
      <button type="button" class="icon-btn" aria-label="Close the import results" onclick={() => libraryStore.closeImport()}>
        <Icon name="close" size={16} />
      </button>
    {/if}
  </div>
  {#if libraryStore.importing && total > 0}
    <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={total} aria-valuenow={done}>
      <span style:width="{(done / total) * 100}%"></span>
    </div>
  {/if}
  <ul class="files">
    {#each files as file (file.index)}
      <li class="file state-{file.outcome?.state ?? 'pending'}">
        <span class="mark">
          {#if !file.outcome}<span class="spinner" aria-hidden="true"></span>
          {:else if file.outcome.state === "imported"}<Icon name="check" size={14} />
          {:else if file.outcome.state === "failed"}<Icon name="warning" size={14} />
          {:else}<Icon name="dash" size={14} />{/if}
        </span>
        <span class="path" title={file.path}>{baseName(file.path)}</span>
        <span class="result muted">
          {#if !file.outcome}
            importing…
          {:else if file.outcome.state === "imported"}
            {#each file.outcome.games as game (game.code)}
              <span class="game">
                {game.name} <span class="mono">{game.code}</span>
                {#if !game.matched}<span class="unmatched">not in the hash database, named after the file</span>{/if}
                {#if game.replaced}<span>replaced an earlier copy</span>{/if}
                {#each game.warnings as warning (warning)}<span class="unmatched">{warning}</span>{/each}
              </span>
            {/each}
          {:else if file.outcome.state === "failed"}
            <span class="error">{file.outcome.error}</span>
          {:else}
            {file.outcome.reason}
          {/if}
        </span>
      </li>
    {/each}
  </ul>
</section>

<style>
  .import {
    border-bottom: 1px solid var(--border);
    padding: var(--space-2) var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-height: 40%;
    background: var(--surface-raised);
    font-size: 0.85rem;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h3 {
    margin: 0;
    font-size: 0.95rem;
  }

  .bar {
    height: 6px;
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

  .files {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
    min-height: 0;
  }

  .file {
    display: grid;
    grid-template-columns: 1.25rem minmax(8rem, 1fr) 2fr;
    gap: var(--space-2);
    align-items: baseline;
    padding: 2px 0;
  }

  .mark {
    display: inline-flex;
    color: var(--text-faint);
  }

  .state-imported .mark {
    color: var(--ok);
  }

  .state-failed .mark {
    color: var(--danger);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 0.8rem;
  }

  .game {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .unmatched {
    color: var(--warn);
  }

  .error {
    color: var(--danger);
  }

  .spinner {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
