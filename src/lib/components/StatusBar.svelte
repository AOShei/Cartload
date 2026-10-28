<script lang="ts">
  import Icon from "./Icon.svelte";
  import { app, consoleStore, flowStore, flowTitle, logStore, preferences } from "../state";

  const report = $derived(consoleStore.report);
  const flow = $derived(flowStore.snapshot);
  /** The running task's position, for the button that reopens its window. */
  const progress = $derived.by((): string => {
    if (flow.current === null) {
      return "";
    }
    return `step ${flow.current + 1} of ${flow.steps.length}`;
  });
  const lineCount = $derived(logStore.visible.length);

  function toggleLog(): void {
    preferences.setLogOpen(!preferences.logOpen);
  }
</script>

<footer class="status" aria-label="Console status">
  <span class="dot tone-{report?.tone ?? 'busy'}" aria-hidden="true"></span>
  <span class="title" role="status">{report?.title ?? "Looking for the console…"}</span>
  {#if flowStore.running && !flowStore.windowOpen}
    <button type="button" class="btn btn-quiet btn-sm task" onclick={() => flowStore.showWindow()} title="Show the progress">
      <span class="spinner" aria-hidden="true"></span>
      {flowTitle(flow.kind)}{progress ? `, ${progress}` : ""}
    </button>
  {:else if flowStore.unresolved && !flowStore.windowOpen}
    <span class="task-result" class:failed={flow.state === "failed"}>
      <button type="button" class="btn btn-quiet btn-sm" onclick={() => flowStore.showWindow()} title="Show what happened">
        <Icon name={flow.state === "failed" ? "warning" : "stop"} size={14} />
        {flowTitle(flow.kind)} {flow.state === "failed" ? "failed" : "stopped"}: details
      </button>
      <button type="button" class="icon-btn icon-btn-sm" aria-label="Dismiss" title="Dismiss" onclick={() => flowStore.dismiss()}>
        <Icon name="close" size={12} />
      </button>
    </span>
  {/if}
  <span class="spacer"></span>
  <button
    type="button"
    class="btn btn-quiet btn-sm log-toggle"
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
    height: var(--control-lg);
    padding: 0 var(--space-3);
    font-size: var(--text-sm);
    background: var(--surface-raised);
    border-top: 1px solid var(--border);
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .task {
    flex: none;
    color: var(--accent-strong);
  }

  .task-result {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--warn);
  }

  .task-result.failed {
    color: var(--danger);
  }

  .task-result .btn {
    color: inherit;
  }

  .log-toggle[aria-pressed="true"] {
    background: var(--surface-hover);
    color: var(--text);
  }

  .count {
    padding: 0 var(--space-1);
    border-radius: 999px;
    background: var(--border);
    font-size: var(--text-xs);
  }

  .version {
    white-space: nowrap;
  }
</style>
