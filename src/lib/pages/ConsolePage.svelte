<script lang="ts">
  import { consoleExec } from "../api/commands";
  import { AppError } from "../api/tauri";
  import type { ExecResult } from "../api/types";
  import DetailsList from "../components/DetailsList.svelte";
  import Icon from "../components/Icon.svelte";
  import StatusCard from "../components/StatusCard.svelte";
  import { app, consoleStore, navigation, toasts } from "../state";

  const report = $derived(consoleStore.report);
  const status = $derived(report?.status ?? null);

  /** The one primary action for the current state. */
  const primary = $derived.by(() => {
    if (!status) {
      return null;
    }
    switch (status.state) {
      case "connected":
        if (status.info.minimal_memboot || !status.info.can_interact) {
          return { label: "Open Kernel", run: () => navigation.go("kernel") };
        }
        return { label: "Open Library", run: () => navigation.go("library") };
      case "fel":
        return status.check.kind === "fel" && status.check.r16
          ? { label: "Install kernel", run: () => navigation.go("kernel") }
          : {
              label: consoleStore.probing ? "Checking…" : "Check FEL mode",
              run: () => void consoleStore.probeFel(),
              busy: consoleStore.probing,
            };
      default:
        return null;
    }
  });

  let command = $state("");
  let running = $state(false);
  let result = $state<ExecResult | null>(null);

  async function runCommand(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!command.trim() || running) {
      return;
    }
    running = true;
    try {
      result = await consoleExec(command);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      running = false;
    }
  }
</script>

<div class="page">
  <div class="main">
    <StatusCard
      {report}
      refreshing={consoleStore.refreshing}
      {primary}
      onrefresh={() => void consoleStore.refresh()}
    />

    {#if report && report.details.length > 0}
      <section class="card details" aria-label="Detected details">
        <h2>Detected</h2>
        <DetailsList details={report.details} />
      </section>
    {/if}

  {#if app.devExec && status?.state === "connected"}
    <section class="card dev" aria-label="Run a command on the console">
      <h2>Run a command <span class="muted">(development build)</span></h2>
      <form onsubmit={runCommand}>
        <input
          type="text"
          class="mono"
          placeholder="hakchi get rootfs"
          bind:value={command}
          aria-label="Shell line to run on the console"
          spellcheck="false"
        />
        <button type="submit" class="btn" disabled={running || !command.trim()}>
          {#if running}
            <span class="spinner" aria-hidden="true"></span>
            Running…
          {:else}
            <Icon name="play" size={16} />
            Run
          {/if}
        </button>
      </form>
      {#if result}
        <p class="muted num">exit code {result.exit_code ?? "none"}</p>
        {#if result.stdout}<pre>{result.stdout}</pre>{/if}
        {#if result.stderr}<pre class="stderr">{result.stderr}</pre>{/if}
      {/if}
    </section>
  {/if}
  </div>

  {#if status}
    <section class="card raw" aria-labelledby="raw-heading">
      <h2 id="raw-heading" class="caps">Everything detected, as data</h2>
      <pre>{JSON.stringify(status, null, 2)}</pre>
    </section>
  {/if}
</div>

<style>
  /* Narrow: one scrolling column, the data in a fixed-height box. Wide (1100 px and up):
     the status card and the table on the left, the data in a panel on the right that
     scrolls inside its column. */
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow: auto;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .details {
    padding: var(--space-4) var(--space-5);
  }

  .details h2 {
    margin-bottom: var(--space-3);
  }

  .raw {
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .raw h2 {
    flex: none;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  .raw pre {
    margin: 0;
    padding: var(--space-3) var(--space-4);
    max-height: 320px;
    overflow: auto;
    font-size: var(--text-sm);
    user-select: text;
  }

  @media (min-width: 1100px) {
    .page {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(340px, 34%);
      grid-template-rows: minmax(0, 1fr);
      gap: var(--space-5);
      overflow: hidden;
    }

    .main {
      max-height: 100%;
      overflow: auto;
    }

    .raw {
      max-height: 100%;
    }

    .raw pre {
      flex: 1;
      min-height: 0;
      max-height: none;
    }
  }

  .dev {
    padding: var(--space-4) var(--space-5);
  }

  .dev form {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  .dev input {
    flex: 1;
  }

  .dev pre {
    margin: var(--space-2) 0 0;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-s);
    white-space: pre-wrap;
    user-select: text;
  }

  .dev .stderr {
    color: var(--danger);
  }
</style>
