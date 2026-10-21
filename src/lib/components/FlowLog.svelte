<script lang="ts">
  import Icon from "./Icon.svelte";
  import { formatTime, logStore, preferences } from "../state";

  interface Props {
    /** Unix milliseconds: only lines from then on are shown. */
    since: number | null;
  }

  let { since }: Props = $props();

  // A little slack: the first lines of a flow can precede the snapshot's start time.
  const lines = $derived(
    since === null ? [] : logStore.visible.filter((line) => line.time >= since - 1500),
  );

  let list: HTMLOListElement | undefined = $state();
  let stickToBottom = $state(true);

  $effect(() => {
    void lines.length;
    if (list && stickToBottom) {
      list.scrollTop = list.scrollHeight;
    }
  });

  function onscroll(): void {
    if (!list) {
      return;
    }
    stickToBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 8;
  }
</script>

<section class="flow-log" aria-label="Log of this run">
  <header>
    <h3>Log</h3>
    <span class="spacer"></span>
    <button
      type="button"
      class="btn btn-quiet small"
      onclick={() => preferences.setLogOpen(true)}
      title="Open the full activity log (Ctrl+L)"
    >
      <Icon name="log" size={14} />
      Full log
    </button>
  </header>
  <ol role="log" bind:this={list} onscroll={onscroll}>
    {#each lines as line (line.seq)}
      <li class="line level-{line.level}">
        <span class="time num">{formatTime(line.time)}</span>
        <span class="source">{line.source}</span>
        <span class="message">{line.message}</span>
      </li>
    {:else}
      <li class="empty">Nothing yet.</li>
    {/each}
  </ol>
</section>

<style>
  .flow-log {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    background: var(--surface-raised);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  h3 {
    margin: 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .spacer {
    flex: 1;
  }

  .small {
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
  }

  ol {
    margin: 0;
    padding: var(--space-1) 0;
    list-style: none;
    max-height: 12rem;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    user-select: text;
  }

  .line {
    display: grid;
    grid-template-columns: 5rem 4.5rem 1fr;
    gap: var(--space-2);
    padding: 1px var(--space-3);
    line-height: 1.5;
  }

  .time,
  .source {
    color: var(--text-muted);
  }

  .level-warn .message {
    color: var(--warn);
  }

  .level-error .message {
    color: var(--danger);
  }

  .level-debug .message,
  .level-trace .message {
    color: var(--text-muted);
  }

  .message {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .empty {
    padding: var(--space-2) var(--space-3);
    color: var(--text-muted);
  }
</style>
