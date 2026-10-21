<script lang="ts">
  import Icon from "./Icon.svelte";
  import { formatTime, logStore, preferences } from "../state";

  interface Props {
    /** Unix milliseconds: only lines from then on are shown. */
    since: number | null;
    /** Fill the parent's remaining height instead of stopping at 192 px. */
    grow?: boolean;
  }

  let { since, grow = false }: Props = $props();

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

<section class="flow-log" class:grow aria-label="Log of this run">
  <header>
    <h3 class="caps">Log</h3>
    <span class="spacer"></span>
    <button
      type="button"
      class="btn btn-quiet btn-sm"
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
      <li class="empty">Nothing yet. The steps report here as they run.</li>
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
    height: var(--control);
    padding: 0 var(--space-1) 0 var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .spacer {
    flex: 1;
  }

  .grow {
    flex: 1 1 160px;
    min-height: 120px;
  }

  .grow ol {
    flex: 1;
    max-height: none;
  }

  ol {
    margin: 0;
    padding: var(--space-1) 0;
    list-style: none;
    min-height: 0;
    max-height: 192px;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    user-select: text;
  }

  .line {
    display: grid;
    grid-template-columns: 80px 72px 1fr;
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
    font-family: var(--font);
  }
</style>
