<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { StepSnapshot } from "../api/types";
  import { formatBytes, formatDuration } from "../state";

  interface Props {
    steps: StepSnapshot[];
    /** Current time, ticking while a flow runs, for the running step's elapsed time. */
    now: number;
  }

  let { steps, now }: Props = $props();

  function timing(step: StepSnapshot): string {
    if (step.state === "running" && step.started_at !== null) {
      return formatDuration(now - step.started_at);
    }
    if (step.elapsed_ms !== null && step.state !== "pending") {
      return formatDuration(step.elapsed_ms);
    }
    return "";
  }

  function stateWord(step: StepSnapshot): string {
    switch (step.state) {
      case "running":
        return "running";
      case "done":
        return "done";
      case "skipped":
        return "skipped";
      case "failed":
        return "failed";
      default:
        return "pending";
    }
  }
</script>

<ol class="steps" aria-label="Steps">
  {#each steps as step (step.index)}
    <li class="step state-{step.state}" aria-current={step.state === "running" ? "step" : undefined}>
      <span class="marker" aria-hidden="true">
        {#if step.state === "running"}
          <span class="spinner"></span>
        {:else if step.state === "done"}
          <Icon name="check" size={14} />
        {:else if step.state === "skipped"}
          <Icon name="dash" size={14} />
        {:else if step.state === "failed"}
          <Icon name="warning" size={14} />
        {:else}
          <span class="dot"></span>
        {/if}
      </span>
      <div class="body">
        <div class="head">
          <span class="label">{step.label}</span>
          <span class="visually-hidden">, {stateWord(step)}</span>
          <span class="time num">{timing(step)}</span>
        </div>
        <p class="desc">{step.description}</p>
        {#if step.transfer}
          <div class="transfer">
            <progress max={step.transfer.total} value={step.transfer.done}></progress>
            <span class="num">
              {formatBytes(step.transfer.done)} of {formatBytes(step.transfer.total)}
            </span>
          </div>
        {/if}
        {#if step.message}
          <p class="message">{step.message}</p>
        {/if}
      </div>
    </li>
  {/each}
</ol>

<style>
  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
  }

  .step {
    position: relative;
    display: grid;
    grid-template-columns: 1.5rem 1fr;
    gap: var(--space-3);
    padding: var(--space-2) 0;
  }

  /* the vertical line between markers */
  .step::before {
    content: "";
    position: absolute;
    left: calc(0.75rem - 1px);
    top: 1.9rem;
    bottom: -0.4rem;
    width: 2px;
    background: var(--border);
  }

  .step:last-child::before {
    display: none;
  }

  .marker {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    background: var(--surface-raised);
    color: var(--text-faint);
    margin-top: 1px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .state-done .marker {
    background: var(--ok-soft);
    color: var(--ok);
  }

  .state-skipped .marker {
    color: var(--text-muted);
  }

  .state-failed .marker {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .state-running .marker {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .spinner {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--accent-soft);
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }

  .body {
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  .label {
    font-weight: 600;
  }

  .state-pending .label {
    color: var(--text-muted);
    font-weight: 500;
  }

  .state-running .label {
    color: var(--accent-strong);
  }

  .state-failed .label {
    color: var(--danger);
  }

  .time {
    margin-left: auto;
    color: var(--text-muted);
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .desc {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .state-pending .desc {
    color: var(--text-faint);
  }

  .message {
    margin-top: var(--space-1);
    font-size: 0.85rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .state-failed .message {
    color: var(--danger);
  }

  .transfer {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-2);
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  progress {
    flex: 1;
    height: 6px;
    accent-color: var(--accent);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
      border-top-color: var(--accent);
      border-right-color: var(--accent);
    }
  }
</style>
