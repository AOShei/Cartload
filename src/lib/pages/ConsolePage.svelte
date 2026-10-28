<script lang="ts">
  import { onMount } from "svelte";
  import { consoleExec } from "../api/commands";
  import { AppError, inTauri } from "../api/tauri";
  import type { ExecResult, StatusReport } from "../api/types";
  import CodeBlock from "../components/console/CodeBlock.svelte";
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

  type Tab = "detected" | "technical" | "command";
  // `?tab=technical` opens that tab in the browser preview.
  const previewTab = !inTauri && typeof location !== "undefined" ? new URLSearchParams(location.search).get("tab") : null;
  let tab = $state<Tab>(previewTab === "technical" || previewTab === "command" ? previewTab : "detected");
  const tabs = $derived.by((): { id: Tab; label: string }[] => {
    const list: { id: Tab; label: string }[] = [
      { id: "detected", label: "Detected" },
      { id: "technical", label: "Technical details" },
    ];
    if (app.devExec && status?.state === "connected") {
      list.push({ id: "command", label: "Command" });
    }
    return list;
  });
  /** The chosen tab, or Detected when the chosen one is gone (the command tab offline). */
  const shown = $derived(tabs.some((t) => t.id === tab) ? tab : "detected");

  /** Left / Right / Home / End move between the tabs and select them. */
  function onTabKey(event: KeyboardEvent): void {
    const index = tabs.findIndex((t) => t.id === shown);
    const last = tabs.length - 1;
    const next =
      event.key === "ArrowRight" ? (index + 1) % tabs.length
      : event.key === "ArrowLeft" ? (index - 1 + tabs.length) % tabs.length
      : event.key === "Home" ? 0
      : event.key === "End" ? last
      : null;
    if (next === null) {
      return;
    }
    event.preventDefault();
    tab = tabs[next]!.id;
    document.getElementById(`tab-${tab}`)?.focus();
  }

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
    <StatusCard {report} refreshing={consoleStore.refreshing} {primary} onrefresh={() => void consoleStore.refresh()} />
    <LinuxSetup onlyWhenNeeded />
    {#if mode === "installed"}
      <MaintenanceCard installed row />
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
    <div class="side-fill">
      {#if mode === "setup"}
        <MaintenanceCard installed={false} />
      {/if}

      <section class="card info">
        <div class="tabs" role="tablist" aria-label="Console information">
          {#each tabs as t (t.id)}
            <button
              type="button"
              role="tab"
              id="tab-{t.id}"
              aria-selected={shown === t.id}
              aria-controls="panel-{t.id}"
              tabindex={shown === t.id ? 0 : -1}
              class:active={shown === t.id}
              onclick={() => (tab = t.id)}
              onkeydown={onTabKey}
            >
              {t.label}
            </button>
          {/each}
        </div>

        <div class="panel" role="tabpanel" id="panel-{shown}" aria-labelledby="tab-{shown}" tabindex="0">
          {#if shown === "detected"}
            {#if report && report.details.length > 0}
              <DetailsList details={report.details} />
            {:else}
              <p class="muted">Nothing detected yet.</p>
            {/if}
          {:else if shown === "technical"}
            {#if status}
              <CodeBlock text={JSON.stringify(status, null, 2)} />
            {:else}
              <p class="muted">No data yet.</p>
            {/if}
          {:else}
            <p class="muted lede">Runs a shell line on the console (development build).</p>
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
              {#if result.stdout}<pre class="out">{result.stdout}</pre>{/if}
              {#if result.stderr}<pre class="out stderr">{result.stderr}</pre>{/if}
            {/if}
          {/if}
        </div>
      </section>
    </div>
  </div>
</div>

<style>
  /* Narrow: one scrolling column, the information panel at a fixed height. Wide (1100 px and
     up): the status card, hakchi and the emulators (or the setup) on the left; on the right
     the information panel, exactly as tall as the left column: it fills its column without
     adding to the row's height, so the left column alone sets it, and its content scrolls
     inside. Both reach the bottom of the window; the page scrolls when the left column is
     taller than that. */
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow: auto;
  }

  .main,
  .side-fill {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .unsupported {
    padding: var(--space-4) var(--space-5);
  }

  .unsupported h2 {
    margin-bottom: var(--space-3);
  }

  .info {
    display: flex;
    flex-direction: column;
    height: 420px;
    min-height: 0;
    overflow: hidden;
  }

  .tabs {
    flex: none;
    display: flex;
    gap: var(--space-1);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  .tabs button {
    position: relative;
    padding: var(--space-3) var(--space-2);
    border: 0;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: var(--weight-semibold);
    cursor: pointer;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.active {
    color: var(--accent-strong);
  }

  .tabs button.active::after {
    content: "";
    position: absolute;
    left: var(--space-1);
    right: var(--space-1);
    bottom: -1px;
    height: 3px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  .panel {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .lede {
    font-size: var(--text-sm);
  }

  form {
    display: flex;
    gap: var(--space-2);
  }

  form input {
    flex: 1;
  }

  .out {
    margin: 0;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-s);
    white-space: pre-wrap;
    user-select: text;
  }

  .stderr {
    color: var(--danger);
  }

  @media (min-width: 1100px) {
    /* The row is at least the page's height, so both columns reach the bottom of the window;
       the left column's last card (Emulators, or the setup's last step) takes the space left
       over. A left column taller than the window makes the row, and the page, taller. */
    .page {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(320px, 32%);
      grid-template-rows: auto;
      align-content: stretch;
      gap: var(--space-5);
    }

    /* The cards come from other components, so the selector reaches past this one's scope. */
    .main > :global(:last-child) {
      flex: 1 0 auto;
    }

    .side {
      position: relative;
      align-self: stretch;
      min-height: 320px;
    }

    .side-fill {
      position: absolute;
      inset: 0;
    }

    .info {
      flex: 1;
      height: auto;
    }
  }
</style>
