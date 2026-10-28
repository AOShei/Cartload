<script lang="ts">
  import { onMount } from "svelte";
  import { consoleExec } from "../api/commands";
  import { AppError } from "../api/tauri";
  import type { ExecResult, StatusReport } from "../api/types";
  import EmulatorsCard from "../components/console/EmulatorsCard.svelte";
  import HakchiSetup from "../components/console/HakchiSetup.svelte";
  import MaintenanceCard from "../components/console/MaintenanceCard.svelte";
  import { canManage } from "../components/console/watch";
  import DetailsList from "../components/DetailsList.svelte";
  import Icon from "../components/Icon.svelte";
  import LinuxSetup from "../components/LinuxSetup.svelte";
  import StatusCard from "../components/StatusCard.svelte";
  import { app, consoleStore, libraryStore, modulesStore, packageStore, toasts } from "../state";

  /* Everything about the console: its state, then either the hakchi setup (a console without
     hakchi) or what a console with hakchi offers (emulators, maintenance). Tasks run in the
     progress window, so this page never changes under the user's hands. */
  onMount(() => {
    void packageStore.start();
    void modulesStore.start();
    void libraryStore.start();
  });

  const report = $derived(consoleStore.report);
  const status = $derived(report?.status ?? null);

  type Mode = "installed" | "setup" | "unsupported";

  /** The mode the console's state calls for, or null while it is in between (rebooting
   *  during a task, unplugged): then the page keeps what it showed. */
  function modeFor(report: StatusReport | null): Mode | null {
    const status = report?.status;
    switch (status?.state) {
      case "connected":
        if (report && !report.supported) {
          return "unsupported";
        }
        return canManage(report) ? "installed" : "setup";
      case "fel":
        return "setup";
      default:
        return null;
    }
  }

  let mode = $state<Mode>(modeFor(consoleStore.report) ?? "setup");
  $effect(() => {
    const next = modeFor(report);
    if (next) {
      mode = next;
    }
  });

  /** The status card's one action: only where the state needs one the page does not show. */
  const primary = $derived.by(() => {
    if (status?.state === "fel" && !(status.check.kind === "fel" && status.check.r16)) {
      return {
        label: consoleStore.probing ? "Checking…" : "Check FEL mode",
        run: () => void consoleStore.probeFel(),
        busy: consoleStore.probing,
      };
    }
    return null;
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
  <StatusCard {report} refreshing={consoleStore.refreshing} {primary} onrefresh={() => void consoleStore.refresh()} />
  <LinuxSetup onlyWhenNeeded />

  <div class="columns">
    <div class="main">
      {#if mode === "installed"}
        <EmulatorsCard />
      {:else if mode === "setup"}
        <HakchiSetup />
      {:else}
        <section class="card unsupported">
          <h2>Not supported</h2>
          <p class="muted">Cartload works with the NES, SNES and Famicom Classic Minis. The Mega Drive Mini is detected only to be left alone.</p>
        </section>
      {/if}
    </div>

    <div class="side">
      {#if mode !== "unsupported"}
        <MaintenanceCard installed={mode === "installed"} />
      {/if}

      {#if report && report.details.length > 0}
        <section class="card details" aria-labelledby="details-heading">
          <h2 id="details-heading">Detected</h2>
          <DetailsList details={report.details} />
        </section>
      {/if}

      {#if app.devExec && status?.state === "connected"}
        <section class="card dev" aria-label="Run a command on the console">
          <h2>Run a command <span class="muted">(development build)</span></h2>
          <form onsubmit={runCommand}>
            <input type="text" class="mono" placeholder="hakchi get rootfs" bind:value={command} aria-label="Shell line to run on the console" spellcheck="false" />
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

      {#if status}
        <details class="card raw">
          <summary>Technical details</summary>
          <p class="muted raw-lede">Raw data from the console, useful in bug reports.</p>
          <pre>{JSON.stringify(status, null, 2)}</pre>
        </details>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Narrow: one scrolling column. Wide (1100 px and up): the main card (emulators, or the
     setup) on the left and the maintenance and details on the right, each column scrolling
     on its own under the status card. */
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow: auto;
  }

  .columns,
  .main,
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .details,
  .unsupported,
  .dev {
    padding: var(--space-4) var(--space-5);
  }

  .details h2,
  .unsupported h2 {
    margin-bottom: var(--space-3);
  }

  .raw {
    padding: var(--space-3) var(--space-5);
  }

  .raw summary {
    cursor: pointer;
    font-weight: var(--weight-semibold);
  }

  .raw-lede {
    margin: var(--space-2) 0;
    font-size: var(--text-sm);
  }

  .raw pre {
    margin: 0;
    max-height: 320px;
    overflow: auto;
    font-size: var(--text-sm);
    user-select: text;
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

  @media (min-width: 1100px) {
    .page {
      overflow: hidden;
    }

    .columns {
      flex: 1;
      min-height: 0;
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(360px, 34%);
      gap: var(--space-5);
    }

    .main,
    .side {
      min-height: 0;
      overflow: auto;
    }
  }
</style>
