<script lang="ts">
  import Cartridge from "./Cartridge.svelte";
  import Icon from "./Icon.svelte";
  import type { StatusReport } from "../api/types";

  interface Action {
    label: string;
    run: () => void;
  }

  interface Props {
    report: StatusReport | null;
    refreshing: boolean;
    /** The one accent-coloured action, when the state calls for one. */
    primary: Action | null;
    onrefresh: () => void;
  }

  let { report, refreshing, primary, onrefresh }: Props = $props();

  const tone = $derived(report?.tone ?? "busy");
  const busy = $derived(tone === "busy" || refreshing);
</script>

<section class="card status-card tone-{tone}" aria-labelledby="status-title">
  <div class="glyph">
    <Cartridge size={56} busy={busy} />
  </div>
  <div class="text">
    <h1 id="status-title">{report?.title ?? "Looking for the console…"}</h1>
    <p class="next">{report?.next ?? ""}</p>
  </div>
  <div class="actions">
    {#if primary}
      <button type="button" class="btn btn-primary" onclick={primary.run}>
        {primary.label}
        <Icon name="arrow" size={16} />
      </button>
    {/if}
    <button
      type="button"
      class="btn btn-quiet"
      onclick={onrefresh}
      disabled={refreshing}
      aria-label="Look again"
      title="Look again (F5)"
    >
      <Icon name="refresh" size={16} />
      <span>{refreshing ? "Checking…" : "Refresh"}</span>
    </button>
  </div>
</section>

<style>
  .status-card {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: var(--space-5);
    align-items: center;
    padding: var(--space-5) var(--space-6);
    border-left: 5px solid var(--text-faint);
  }

  .tone-ready {
    border-left-color: var(--ok);
  }

  .tone-busy {
    border-left-color: var(--accent);
  }

  .tone-warning {
    border-left-color: var(--warn);
  }

  .tone-error {
    border-left-color: var(--danger);
  }

  .glyph {
    color: var(--text-faint);
    display: flex;
  }

  .tone-ready .glyph {
    color: var(--ok);
  }

  .tone-busy .glyph {
    color: var(--accent);
  }

  .tone-warning .glyph {
    color: var(--warn);
  }

  .tone-error .glyph {
    color: var(--danger);
  }

  .text {
    min-width: 0;
  }

  .next {
    margin-top: var(--space-2);
    color: var(--text-muted);
    max-width: 60ch;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: stretch;
  }

  @media (max-width: 860px) {
    .status-card {
      grid-template-columns: auto 1fr;
    }

    .actions {
      grid-column: 1 / -1;
      flex-direction: row;
    }
  }
</style>
