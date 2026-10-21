<script lang="ts">
  import Icon from "./Icon.svelte";
  import { app, consoleStore, logStore, preferences } from "../state";

  const report = $derived(consoleStore.report);
  const lineCount = $derived(logStore.visible.length);

  function toggleLog(): void {
    preferences.setLogOpen(!preferences.logOpen);
  }
</script>

<footer class="status" aria-label="Console status">
  <span class="dot tone-{report?.tone ?? 'busy'}" aria-hidden="true"></span>
  <span class="title" role="status">{report?.title ?? "Looking for the console…"}</span>
  <span class="spacer"></span>
  <button
    type="button"
    class="btn btn-quiet log-toggle"
    aria-pressed={preferences.logOpen}
    aria-label={preferences.logOpen ? "Hide the activity log" : "Show the activity log"}
    title="Activity log (Ctrl+L)"
    onclick={toggleLog}
  >
    <Icon name="log" size={16} />
    <span>Log</span>
    <span class="count num">{lineCount}</span>
  </button>
  <span class="version muted num" title={app.version?.crates.join("\n")}>
    {#if app.version}
      Cartload {app.version.app}{app.version.debug ? " (debug)" : ""}
    {:else if app.versionError}
      Cartload ({app.versionError})
    {:else}
      Cartload
    {/if}
  </span>
</footer>

<style>
  .status {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-1) var(--space-3);
    font-size: 0.8125rem;
    background: var(--surface-raised);
    border-top: 1px solid var(--border);
    min-height: 2.25rem;
  }

  .dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    flex: none;
  }

  .tone-neutral {
    background: var(--text-faint);
  }
  .tone-ready {
    background: var(--ok);
  }
  .tone-busy {
    background: var(--accent);
  }
  .tone-warning {
    background: var(--warn);
  }
  .tone-error {
    background: var(--danger);
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .log-toggle {
    padding: 0.2rem 0.5rem;
    font-size: inherit;
  }

  .log-toggle[aria-pressed="true"] {
    background: var(--surface-hover);
    color: var(--text);
  }

  .count {
    padding: 0 0.35rem;
    border-radius: 999px;
    background: var(--border);
    font-size: 0.7rem;
  }

  .version {
    white-space: nowrap;
  }
</style>
