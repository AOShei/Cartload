<script lang="ts">
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import type { ImportEvent, ImportedGame } from "../../api/types";
  import { baseName, libraryStore } from "../../state";

  /* A strip above the grid while files import: a progress bar with running counts of games
     added, warnings and failures. When the import ends the strip keeps the totals and offers
     the full report in a dialog, file by file, failures and warnings first. */

  const files = $derived(libraryStore.importFiles);
  const done = $derived(files.filter((f) => f.outcome !== null).length);
  const total = $derived(files[0]?.total ?? files.length);
  let reportOpen = $state(false);

  /** A finished file, classified for the counters and the report. */
  interface Row {
    file: ImportEvent;
    kind: "failed" | "warning" | "skipped" | "added";
    games: ImportedGame[];
  }

  const rows = $derived.by((): Row[] =>
    files.flatMap((file): Row[] => {
      const outcome = file.outcome;
      if (!outcome) {
        return [];
      }
      if (outcome.state === "failed") {
        return [{ file, kind: "failed", games: [] }];
      }
      if (outcome.state === "skipped") {
        return [{ file, kind: "skipped", games: [] }];
      }
      const warned = outcome.games.some((g) => g.warnings.length > 0);
      return [{ file, kind: warned ? "warning" : "added", games: outcome.games }];
    }),
  );

  const added = $derived(rows.reduce((n, r) => n + r.games.length, 0));
  const warnings = $derived(rows.reduce((n, r) => n + r.games.reduce((m, g) => m + g.warnings.length, 0), 0));
  const failed = $derived(rows.filter((r) => r.kind === "failed").length);
  const skipped = $derived(rows.filter((r) => r.kind === "skipped").length);
  const unmatched = $derived(rows.reduce((n, r) => n + r.games.filter((g) => !g.matched).length, 0));

  /** Failures first, then games with warnings, then the rest in file order. */
  const order: Record<Row["kind"], number> = { failed: 0, warning: 1, skipped: 2, added: 3 };
  const sorted = $derived([...rows].sort((a, b) => order[a.kind] - order[b.kind] || a.file.index - b.file.index));

  const heading = $derived.by((): string => {
    if (libraryStore.importing) {
      return `Adding games, ${done} of ${total}`;
    }
    return `${added} game${added === 1 ? "" : "s"} added`;
  });

  function plural(n: number, word: string): string {
    return `${n} ${word}${n === 1 ? "" : "s"}`;
  }
</script>

<section class="import" aria-live="polite" aria-label="Import progress" aria-busy={libraryStore.importing}>
  <div class="head">
    <strong class="heading">{heading}</strong>
    <span class="counts">
      <span class="count ok" title="Games added"><Icon name="check" size={14} />{added}</span>
      <span class="count" class:warn={warnings > 0} title="Warnings"><Icon name="warning" size={14} />{warnings}</span>
      <span class="count" class:bad={failed > 0} title="Files that failed"><Icon name="close" size={14} />{failed}</span>
    </span>
    {#if !libraryStore.importing}
      <button type="button" class="btn btn-sm" onclick={() => (reportOpen = true)} disabled={rows.length === 0}>Show report</button>
      <button type="button" class="icon-btn icon-btn-sm" aria-label="Dismiss the import summary" onclick={() => libraryStore.closeImport()}>
        <Icon name="close" size={16} />
      </button>
    {/if}
  </div>
  {#if libraryStore.importing}
    <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={Math.max(total, 1)} aria-valuenow={done}>
      <span style:width="{total > 0 ? (done / total) * 100 : 0}%"></span>
    </div>
  {/if}
</section>

<Dialog open={reportOpen} title="Import report" width="720px" onclose={() => (reportOpen = false)}>
  <p class="muted summary">
    {plural(added, "game")} added from {plural(total, "file")}.
    {#if warnings > 0}{plural(warnings, "warning")}.{/if}
    {#if failed > 0}{plural(failed, "file")} failed.{/if}
    {#if skipped > 0}{plural(skipped, "file")} skipped.{/if}
    {#if unmatched > 0}{unmatched} not in the hash database and named after the file.{/if}
  </p>
  <ol class="report">
    {#each sorted as row (row.file.index)}
      <li class="row kind-{row.kind}">
        <span class="mark" aria-hidden="true">
          {#if row.kind === "failed"}<Icon name="close" size={14} />
          {:else if row.kind === "warning"}<Icon name="warning" size={14} />
          {:else if row.kind === "skipped"}<Icon name="dash" size={14} />
          {:else}<Icon name="check" size={14} />{/if}
        </span>
        <div class="body">
          <span class="path" title={row.file.path}>{baseName(row.file.path)}</span>
          {#if row.file.outcome?.state === "failed"}
            <span class="text bad">Failed: {row.file.outcome.error}</span>
          {:else if row.file.outcome?.state === "skipped"}
            <span class="text muted">Skipped: {row.file.outcome.reason}</span>
          {:else}
            {#each row.games as game (game.code)}
              <span class="game">
                <span>{game.name} <span class="mono muted">{game.code}</span></span>
                {#each game.warnings as warning (warning)}<span class="text warn">Warning: {warning}</span>{/each}
                {#if !game.matched}<span class="text muted">Not in the hash database; named after the file.</span>{/if}
                {#if game.replaced}<span class="text muted">Replaced an earlier copy with the same code.</span>{/if}
                {#each game.notes as note (note)}<span class="text muted">{note[0]?.toUpperCase()}{note.slice(1)}.</span>{/each}
              </span>
            {/each}
          {/if}
        </div>
      </li>
    {/each}
  </ol>
  {#snippet actions()}
    <button type="button" class="btn" data-autofocus onclick={() => (reportOpen = false)}>Close</button>
  {/snippet}
</Dialog>

<style>
  .import {
    border-bottom: 1px solid var(--border);
    padding: var(--space-2) var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    background: var(--surface-raised);
    font-size: var(--text-md);
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--control-sm);
  }

  .heading {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Three figures with icons: added, warnings, failures. */
  .counts {
    display: flex;
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .count {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .count.ok :global(svg) {
    color: var(--ok);
  }

  .count.warn {
    color: var(--warn);
  }

  .count.bad {
    color: var(--danger);
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
    transition: width var(--motion-fast) linear;
  }

  .summary {
    margin: 0;
  }

  .report {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .row {
    display: grid;
    grid-template-columns: 20px 1fr;
    gap: var(--space-2);
    align-items: start;
  }

  .mark {
    display: inline-flex;
    margin-top: 3px;
    color: var(--ok);
  }

  .kind-failed .mark {
    color: var(--danger);
  }

  .kind-warning .mark {
    color: var(--warn);
  }

  .kind-skipped .mark {
    color: var(--text-faint);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .path {
    font-weight: var(--weight-semibold);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .game {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .text {
    font-size: var(--text-sm);
  }

  .warn {
    color: var(--warn);
  }

  .bad {
    color: var(--danger);
  }
</style>
