<script lang="ts">
  import Icon from "./Icon.svelte";
  import { formatTime, logStore, preferences, toasts } from "../state";

  let list: HTMLOListElement | undefined = $state();
  let stickToBottom = $state(true);

  const lines = $derived(logStore.visible);

  $effect(() => {
    // Follow new lines unless the user scrolled up.
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

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(logStore.asText());
      toasts.success("Log copied.");
    } catch {
      toasts.error("Could not copy the log to the clipboard.");
    }
  }

  function clear(): void {
    const count = logStore.lines.length;
    if (count === 0) {
      return;
    }
    logStore.clear();
    toasts.undoable(`Log cleared (${count} line${count === 1 ? "" : "s"}).`, () => logStore.undoClear());
  }

  function close(): void {
    preferences.setLogOpen(false);
  }
</script>

<section class="panel" aria-label="Activity log">
  <header>
    <h2 class="caps">Activity</h2>
    <label class="debug">
      <input
        type="checkbox"
        checked={preferences.showDebugLog}
        onchange={(e) => preferences.setShowDebugLog(e.currentTarget.checked)}
      />
      Show details
    </label>
    <span class="spacer"></span>
    <button type="button" class="icon-btn" onclick={copy} aria-label="Copy the log" title="Copy" disabled={lines.length === 0}>
      <Icon name="copy" size={16} />
    </button>
    <button
      type="button"
      class="icon-btn"
      onclick={clear}
      aria-label="Clear the log"
      title="Clear"
      disabled={lines.length === 0}
    >
      <Icon name="trash" size={16} />
    </button>
    <button type="button" class="icon-btn" onclick={close} aria-label="Hide the activity log" title="Hide (Ctrl+L)">
      <Icon name="close" size={16} />
    </button>
  </header>
  <ol role="log" bind:this={list} onscroll={onscroll}>
    {#each lines as line (line.seq)}
      <li class="line level-{line.level}">
        <span class="time num">{formatTime(line.time)}</span>
        <span class="level">{line.level}</span>
        <span class="source">{line.source}</span>
        <span class="message">{line.message}</span>
      </li>
    {:else}
      <li class="empty muted">
        Nothing yet. What the app does with the console appears here as it happens{preferences.showDebugLog
          ? "."
          : "; tick “Show details” for every command and USB event."}
      </li>
    {/each}
  </ol>
</section>

<style>
  /* Fixed height: the panel shrinks the page content area, never pushes it off screen.
     Short windows get a shorter log so the page keeps at least half of the height. */
  .panel {
    flex: none;
    display: flex;
    flex-direction: column;
    height: clamp(144px, 28vh, 224px);
    min-height: 0;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--control-lg);
    padding: 0 var(--space-2) 0 var(--space-3);
    border-bottom: 1px solid var(--border);
    font-size: var(--text-md);
  }

  .debug {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--text-muted);
  }

  .spacer {
    flex: 1;
  }

  ol {
    flex: 1;
    margin: 0;
    padding: var(--space-1) 0;
    list-style: none;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    user-select: text;
  }

  .line {
    display: grid;
    grid-template-columns: 80px 56px 88px 1fr;
    gap: var(--space-2);
    padding: 1px var(--space-3);
    line-height: 1.5;
  }

  .line:hover {
    background: var(--surface-raised);
  }

  .time,
  .source {
    color: var(--text-muted);
  }

  .level {
    text-transform: uppercase;
    font-size: var(--text-xs);
    letter-spacing: 0.04em;
    align-self: center;
  }

  .level-debug .level,
  .level-trace .level {
    color: var(--text-faint);
  }

  .level-info .level {
    color: var(--accent);
  }

  .level-warn .level {
    color: var(--warn);
  }

  .level-error .level {
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
    font-family: var(--font);
  }
</style>
