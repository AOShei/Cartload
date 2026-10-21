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
  let list: HTMLOListElement | undefined = $state();

  /* While a flow runs, the running step and the one after it stay in view. */
  const runningIndex = $derived(steps.findIndex((s) => s.state === "running"));
  $effect(() => {
    const index = runningIndex;
    if (index < 0 || !list) {
      return;
    }
    const items = list.querySelectorAll<HTMLElement>("li");
    const next = items[Math.min(index + 1, items.length - 1)];
    const current = items[index];
    next?.scrollIntoView({ block: "nearest" });
    current?.scrollIntoView({ block: "nearest" });
  });

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

<ol class="steps" aria-label="Steps" bind:this={list}>
  {#each steps as step (step.index)}
    <li class="step state-{step.state}" aria-current={step.state === "running" ? "step" : undefined}>
      <span class="marker" aria-hidden="true">
        {#if step.state === "running"}
          <span class="spinner spinner-sm"></span>
        {:else if step.state === "done"}
          <Icon name="check" size={14} />
        {:else if step.state === "skipped"}
          <Icon name="dash" size={14} />
        {:else if step.state === "failed"}
          <Icon name="warning" size={14} />
        {:else}
          <span class="pending-dot"></span>
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
    grid-template-columns: 24px 1fr;
    gap: var(--space-3);
    padding: var(--space-2) 0;
  }

  /* the vertical line between markers */
  .step::before {
    content: "";
    position: absolute;
    left: 11px;
    top: 32px;
    bottom: -6px;
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
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--surface-raised);
    color: var(--text-faint);
    margin-top: 1px;
    transition:
      background-color var(--motion) var(--ease-out),
      color var(--motion) var(--ease-out);
  }

  .pending-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .state-done .marker {
    background: var(--ok-soft);
    color: var(--ok);
    animation: pop var(--motion-slow) var(--ease-out);
  }

  .state-skipped .marker {
    color: var(--text-muted);
  }

  .state-failed .marker {
    background: var(--danger-soft);
    color: var(--danger);
    animation: pop var(--motion-slow) var(--ease-out);
  }

  .state-running .marker {
    background: var(--accent-soft);
    color: var(--accent);
  }

  @keyframes pop {
    0% {
      transform: scale(0.7);
    }
    60% {
      transform: scale(1.15);
    }
    100% {
      transform: scale(1);
    }
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
    font-weight: var(--weight-semibold);
    transition: color var(--motion) var(--ease-out);
  }

  .state-pending .label {
    color: var(--text-muted);
    font-weight: var(--weight-medium);
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
    font-size: var(--text-md);
    white-space: nowrap;
  }

  .desc {
    color: var(--text-muted);
    font-size: var(--text-md);
  }

  .state-pending .desc {
    color: var(--text-faint);
  }

  .message {
    margin-top: var(--space-1);
    font-size: var(--text-sm);
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
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  .transfer progress {
    flex: 1;
    width: auto;
  }
</style>
