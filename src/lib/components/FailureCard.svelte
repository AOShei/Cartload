<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { FlowFailure } from "../api/types";

  interface Props {
    failure: FlowFailure;
    /** Label of the step that failed, when known. */
    stepLabel: string | null;
    busy: boolean;
    onretry: () => void;
    /** Offered when the failure asks for a stock kernel dump. */
    onchoosekernel?: () => void;
    /** Offered when the chosen kernel file is not a known image. */
    onforce?: () => void;
  }

  let { failure, stepLabel, busy, onretry, onchoosekernel, onforce }: Props = $props();
</script>

<section class="card failure" aria-labelledby="failure-title">
  <div class="icon"><Icon name="warning" size={28} /></div>
  <div class="text">
    <h2 id="failure-title">{failure.title}</h2>
    {#if stepLabel}
      <p class="step">Failed at: {stepLabel}</p>
    {/if}
    <p><span class="k">Likely cause</span> {failure.cause}</p>
    <p><span class="k">What to do</span> {failure.next}</p>
    <details>
      <summary>Raw error</summary>
      <pre>{failure.detail}</pre>
    </details>
  </div>
  <div class="actions">
    {#if failure.wants_stock_kernel && onchoosekernel}
      <button type="button" class="btn btn-primary" onclick={onchoosekernel} disabled={busy}>
        Choose a kernel dump…
      </button>
      <button type="button" class="btn btn-quiet" onclick={onretry} disabled={busy}>
        <Icon name="refresh" size={16} />
        Try again
      </button>
    {:else if failure.wants_force && onforce}
      <button type="button" class="btn btn-primary" onclick={onforce} disabled={busy}>
        Flash it anyway
      </button>
      <button type="button" class="btn btn-quiet" onclick={onretry} disabled={busy}>
        <Icon name="refresh" size={16} />
        Try again
      </button>
    {:else}
      <button type="button" class="btn btn-primary" onclick={onretry} disabled={busy}>
        <Icon name="refresh" size={16} />
        Try again
      </button>
    {/if}
  </div>
</section>

<style>
  .failure {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: var(--space-4);
    align-items: start;
    padding: var(--space-4) var(--space-5);
    border-left: 5px solid var(--danger);
  }

  .icon {
    color: var(--danger);
    display: flex;
    margin-top: 2px;
  }

  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .step {
    color: var(--text-muted);
    font-size: var(--text-md);
  }

  .k {
    display: inline-block;
    min-width: 120px;
    color: var(--text-muted);
  }

  details {
    font-size: var(--text-md);
  }

  pre {
    margin: var(--space-2) 0 0;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-s);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: stretch;
  }

  @media (max-width: 1023px) {
    .failure {
      grid-template-columns: auto 1fr;
    }

    .actions {
      grid-column: 1 / -1;
      flex-direction: row;
    }
  }
</style>
