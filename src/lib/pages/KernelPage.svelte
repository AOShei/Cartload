<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { inTauri } from "../api/tauri";
  import type { FlowKind } from "../api/types";
  import Dialog from "../components/Dialog.svelte";
  import FailureCard from "../components/FailureCard.svelte";
  import ConsoleTopView from "../components/ConsoleTopView.svelte";
  import FlowLog from "../components/FlowLog.svelte";
  import Icon from "../components/Icon.svelte";
  import StepList from "../components/StepList.svelte";
  import {
    consoleStore,
    flowStore,
    flowTitle,
    formatBytes,
    formatDuration,
    navigation,
    packageStore,
    toasts,
  } from "../state";

  onMount(() => {
    void flowStore.start();
    void packageStore.start();
  });

  const report = $derived(consoleStore.report);
  const status = $derived(report?.status ?? null);

  /* The drawing follows the console that is connected, and keeps showing the last one seen
     while it is off or in FEL mode. `?model=` picks one in the browser preview. */
  let lastModel = $state(
    (!inTauri && typeof location !== "undefined" && new URLSearchParams(location.search).get("model")) || "SnesEur",
  );
  $effect(() => {
    if (status?.state === "connected" && status.info.console !== "Unknown") {
      lastModel = status.info.console;
    }
  });
  const consoleModel = $derived(lastModel);
  const snapshot = $derived(flowStore.snapshot);
  const running = $derived(flowStore.running);
  const pkg = $derived(packageStore.status?.package ?? null);
  const busy = $derived(running || flowStore.starting);

  type Tone = "neutral" | "ready" | "busy" | "warning" | "error";

  interface Watch {
    tone: Tone;
    text: string;
    ready: boolean;
    installed: boolean;
  }

  /** Stage 1: what the watcher sees, as one line. */
  const watch = $derived.by((): Watch => {
    const waiting: Watch = {
      tone: "neutral",
      text: "Waiting for the console on USB…",
      ready: false,
      installed: false,
    };
    if (!status) {
      return waiting;
    }
    switch (status.state) {
      case "fel":
        switch (status.check.kind) {
          case "fel":
            return status.check.r16
              ? { tone: "ready", text: "Console in FEL mode. Ready.", ready: true, installed: false }
              : {
                  tone: "warning",
                  text: `A FEL device is attached, but its chip is ${status.check.soc}, not a Classic Mini's.`,
                  ready: false,
                  installed: false,
                };
          case "not_fel":
            return {
              tone: "warning",
              text: "Console booted normally. Switch it off and follow the steps.",
              ready: false,
              installed: false,
            };
          case "unavailable":
            return { tone: "error", text: report?.title ?? "", ready: false, installed: false };
          default:
            return { tone: "busy", text: "Checking the attached console…", ready: false, installed: false };
        }
      case "connected":
        if (status.info.minimal_memboot) {
          return {
            tone: "ready",
            text: "Console running hakchi from memory. Ready.",
            ready: true,
            installed: false,
          };
        }
        return {
          tone: "ready",
          text: "Console connected and running hakchi. Ready, no Reset needed.",
          ready: true,
          installed: true,
        };
      case "booting":
        return { tone: "busy", text: report?.title ?? "Console coming up…", ready: false, installed: false };
      case "working":
        return { tone: "busy", text: "In use by the running flow.", ready: true, installed: false };
      default:
        return waiting;
    }
  });

  const canStart = $derived(watch.ready && pkg !== null && !busy && status?.state !== "working");

  /** Why the primary action is disabled, in one short line. */
  const blocker = $derived.by((): string | null => {
    if (busy) {
      return null;
    }
    if (!watch.ready) {
      return "Waiting for the console (stage 1).";
    }
    if (!pkg) {
      return "Download the kernel package (stage 2) first.";
    }
    return null;
  });

  const stageTitle = $derived(snapshot.state === "idle" ? "Install hakchi" : flowTitle(snapshot.kind));

  const flowElapsed = $derived.by((): string => {
    if (snapshot.started_at === null) {
      return "";
    }
    const end = snapshot.finished_at ?? flowStore.now;
    return formatDuration(end - snapshot.started_at);
  });

  const failedStepLabel = $derived.by((): string | null => {
    const index = snapshot.failure?.step_index;
    if (index === null || index === undefined) {
      return null;
    }
    return snapshot.steps[index]?.label ?? null;
  });

  /** Stage 4: the outcome in one heading and one sentence. */
  const outcome = $derived.by((): { heading: string; text: string } | null => {
    if (snapshot.state !== "done") {
      return null;
    }
    switch (snapshot.kind?.kind) {
      case "memboot":
        return {
          heading: "The console runs hakchi from memory.",
          text: "Nothing was written to it. Switch it off and on to return to its own system.",
        };
      case "uninstall":
        return {
          heading: "The console is back on its original system.",
          text: "hakchi was removed and the original kernel flashed back. The console boots its own menu now.",
        };
      case "dump_kernel":
        return {
          heading: "The stock kernel was saved.",
          text: `Written to ${snapshot.kind.path}. Keep it: it is what returns the console to its original system if the backup on the console is ever lost.`,
        };
      default:
        return {
          heading: "hakchi is installed.",
          text: "The console rebooted into its menu with hakchi running. Add games from the Library page.",
        };
    }
  });

  /** After an install that left the console connected the Library is the next step. */
  const libraryNext = $derived(
    snapshot.state === "done" && snapshot.kind?.kind === "install" && status?.state === "connected",
  );

  let confirmUninstall = $state(false);

  async function begin(kind: FlowKind): Promise<void> {
    confirmUninstall = false;
    await flowStore.begin(kind);
  }

  function retry(): void {
    const kind = snapshot.kind ?? { kind: "install" };
    void begin(kind);
  }

  async function dumpKernel(): Promise<void> {
    if (!inTauri) {
      toasts.info("The save dialog needs the desktop app.");
      return;
    }
    const path = await save({
      title: "Save the stock kernel",
      defaultPath: "kernel.img",
      filters: [{ name: "Kernel image", extensions: ["img"] }],
    });
    if (path) {
      await begin({ kind: "dump_kernel", path });
    }
  }

  async function chooseStockKernel(): Promise<void> {
    if (!inTauri) {
      toasts.info("The file dialog needs the desktop app.");
      return;
    }
    const picked = await open({
      title: "Choose a stock kernel dump",
      multiple: false,
      directory: false,
      filters: [{ name: "Kernel image", extensions: ["img"] }],
    });
    if (typeof picked === "string") {
      await begin({ kind: "uninstall", stock_kernel: picked, force_stock_kernel: false });
    }
  }

  function forceStockKernel(): void {
    const kind = snapshot.kind;
    if (kind?.kind === "uninstall") {
      void begin({ kind: "uninstall", stock_kernel: kind.stock_kernel ?? null, force_stock_kernel: true });
    }
  }

  function modified(ms: number | null): string {
    if (ms === null) {
      return "";
    }
    return new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  }
</script>

<div class="page">
  <div class="columns">
  <ol class="stages left">
    <!-- Stage 1 -->
    <li class="stage" class:complete={watch.ready}>
      <span class="number" aria-hidden="true">{#if watch.ready}<Icon name="check" size={16} />{:else}1{/if}</span>
      <div class="body card">
        <h2>Put the console in FEL mode</h2>
        <div class="fel">
          <ConsoleTopView model={consoleModel} width={250} />
          <ol class="howto">
            <li>Connect the console to this computer with its USB cable and switch POWER off.</li>
            <li>Hold RESET down and switch POWER on.</li>
            <li>Keep RESET held for two seconds, then release it. The screen stays black: that is FEL mode.</li>
          </ol>
        </div>
        <p class="watch" role="status">
          <span class="dot tone-{watch.tone}" aria-hidden="true"></span>
          {watch.text}
        </p>
      </div>
    </li>

    <!-- Stage 2 -->
    <li class="stage" class:complete={pkg !== null}>
      <span class="number" aria-hidden="true">{#if pkg}<Icon name="check" size={16} />{:else}2{/if}</span>
      <div class="body card">
        <div class="row">
          <h2>Kernel package</h2>
          <span class="spacer"></span>
          {#if packageStore.loading && !packageStore.status}
            <span class="muted"><span class="spinner" aria-hidden="true"></span> Checking the cache…</span>
          {:else if packageStore.updating}
            <span class="muted num">
              {#if packageStore.progress?.total}
                {formatBytes(packageStore.progress.done)} of {formatBytes(packageStore.progress.total)}
              {:else}
                Downloading…
              {/if}
            </span>
          {:else if pkg}
            <button type="button" class="btn btn-quiet" onclick={() => void packageStore.update()} disabled={busy}>
              <Icon name="download" size={16} />
              Update package
            </button>
          {/if}
        </div>
        {#if pkg}
          <p class="version">
            hakchi <strong class="num">{pkg.version}</strong>
            {#if pkg.boot || pkg.kernel}
              <span class="muted num">
                (boot {pkg.boot ?? "?"}, kernel {pkg.kernel ?? "?"})
              </span>
            {/if}
          </p>
          <p class="muted small num">
            {formatBytes(pkg.len)}{pkg.modified ? `, downloaded ${modified(pkg.modified)}` : ""}
            {pkg.source === "override" ? " (from CARTLOAD_HMOD)" : ""}
          </p>
          {#if packageStore.error}
            <p class="problem">The update failed: {packageStore.error} The copy on disk is still usable.</p>
          {/if}
        {:else if packageStore.updating}
          <p class="muted">Fetching the latest hakchi.hmod…</p>
          <progress max={packageStore.progress?.total ?? 1} value={packageStore.progress?.total ? packageStore.progress.done : undefined}></progress>
        {:else}
          <p>No kernel package yet. It is a 9 MB download from the hakchi project's servers.</p>
          {#if packageStore.status?.problem}
            <p class="problem">The copy on disk cannot be used: {packageStore.status.problem}</p>
          {/if}
          {#if packageStore.error}
            <p class="problem">The download failed: {packageStore.error}</p>
          {/if}
          <div>
            <button type="button" class="btn btn-primary" onclick={() => void packageStore.update()} disabled={busy}>
              <Icon name="download" size={16} />
              {packageStore.error ? "Try the download again" : "Download package"}
            </button>
          </div>
        {/if}
      </div>
    </li>

    <!-- Stage 3 -->
    <li class="stage" class:complete={snapshot.state === "done"} class:active={running}>
      <span class="number" aria-hidden="true">{#if snapshot.state === "done"}<Icon name="check" size={16} />{:else}3{/if}</span>
      <div class="body card">
        <div class="row">
          <h2>{stageTitle}</h2>
          {#if flowElapsed}
            <span class="muted num elapsed">{flowElapsed}</span>
          {/if}
          <span class="spacer"></span>
          {#if running}
            <button type="button" class="btn btn-quiet" onclick={() => void flowStore.cancel()} disabled={flowStore.cancelling || snapshot.cancel_pending}>
              <Icon name="stop" size={16} />
              {snapshot.cancel_pending ? "Stopping after this step…" : flowStore.cancelling ? "Stopping…" : "Stop"}
            </button>
          {:else if snapshot.state === "idle"}
            <button type="button" class="btn btn-primary" onclick={() => void begin({ kind: "install" })} disabled={!canStart}>
              {#if flowStore.starting}
                <span class="spinner on-accent" aria-hidden="true"></span>
                Starting…
              {:else}
                {watch.installed ? "Reinstall hakchi" : "Install hakchi"}
                <Icon name="arrow" size={16} />
              {/if}
            </button>
          {:else if snapshot.state === "done"}
            <button type="button" class="btn btn-quiet" onclick={() => void begin({ kind: "install" })} disabled={!canStart}>
              <Icon name="refresh" size={16} />
              Install again
            </button>
          {/if}
        </div>
        {#if blocker && snapshot.state === "idle"}
          <p class="muted small">{blocker}</p>
        {/if}
        {#if snapshot.cancel_pending}
          <p class="note">The current step writes to the console's flash, so it finishes first. Do not unplug the console.</p>
        {/if}
      </div>
    </li>

    <!-- Stage 4 -->
    <li class="stage" class:complete={snapshot.state === "done"} class:failed={snapshot.state === "failed"}>
      <span class="number" aria-hidden="true">
        {#if snapshot.state === "done"}<Icon name="check" size={16} />{:else if snapshot.state === "failed"}<Icon name="warning" size={16} />{:else}4{/if}
      </span>
      <div class="body">
        {#if outcome}
          <section class="card result">
            <h2>{outcome.heading}</h2>
            <p class="muted">{outcome.text}</p>
            <p class="summary" role="status">
              <span class="dot tone-{report?.tone ?? 'neutral'}" aria-hidden="true"></span>
              <span class="summary-text">{report?.title ?? "Looking at the console…"}</span>
              {#if flowElapsed}<span class="muted num">{flowElapsed}</span>{/if}
            </p>
            <div class="result-actions">
              <button type="button" class="btn" class:btn-primary={!libraryNext} onclick={() => navigation.go("console")}>Open Console</button>
              <button type="button" class="btn" class:btn-primary={libraryNext} onclick={() => navigation.go("library")}>Open Library</button>
            </div>
          </section>
        {:else if snapshot.state === "failed" && snapshot.failure}
          <FailureCard
            failure={snapshot.failure}
            stepLabel={failedStepLabel}
            busy={busy}
            onretry={retry}
            onchoosekernel={chooseStockKernel}
            onforce={forceStockKernel}
          />
        {:else if snapshot.state === "cancelled"}
          <section class="card result">
            <h2>Stopped.</h2>
            <p class="muted">
              Nothing more was changed. Start again to continue: finished steps are skipped.
            </p>
            <div>
              <button type="button" class="btn btn-primary" onclick={retry} disabled={!canStart}>
                <Icon name="play" size={16} />
                Continue
              </button>
            </div>
          </section>
        {:else}
          <section class="card result placeholder">
            <h2 class="muted">Result</h2>
            <p class="muted">The console's state after the install appears here.</p>
          </section>
        {/if}
      </div>
    </li>
  </ol>

  <div class="right">
    <section class="card progress" aria-labelledby="steps-heading">
      <div class="progress-head">
        <h2 id="steps-heading">Install steps</h2>
        {#if snapshot.state !== "idle" && flowElapsed}
          <span class="muted num">{flowElapsed}</span>
        {/if}
      </div>
      <div class="steps-scroll">
        <StepList steps={flowStore.steps} now={flowStore.now} />
      </div>
      {#if snapshot.state !== "idle"}
        <FlowLog since={snapshot.started_at} grow />
      {/if}
    </section>

    <section class="card tools" aria-labelledby="tools-heading">
      <h2 id="tools-heading">Other actions</h2>
      <ul class="tools-list">
        <li>
          <div class="tool-text">
            <strong>Boot from memory only</strong>
            <span class="muted">Nothing is written.</span>
          </div>
          <button type="button" class="btn" disabled={!canStart} onclick={() => void begin({ kind: "memboot" })}>
            <Icon name="play" size={16} />
            Boot
          </button>
        </li>
        <li>
          <div class="tool-text">
            <strong>Save the stock kernel</strong>
            <span class="muted">Backup to a file.</span>
          </div>
          <button type="button" class="btn" disabled={!canStart} onclick={() => void dumpKernel()}>
            <Icon name="save" size={16} />
            Save…
          </button>
        </li>
        <li>
          <div class="tool-text">
            <strong>Return to the original system</strong>
            <span class="muted">Removes hakchi.</span>
          </div>
          <button type="button" class="btn" disabled={!canStart} onclick={() => (confirmUninstall = true)}>
            <Icon name="undo" size={16} />
            Return…
          </button>
        </li>
      </ul>
    </section>
  </div>
  </div>
</div>

<Dialog open={confirmUninstall} title="Return the console to its original system" width="520px" onclose={() => (confirmUninstall = false)}>
  <p>
    This flashes the original kernel back and removes hakchi from the console. Games added
    with hakchi will no longer appear; the console's own games are untouched.
  </p>
  {#snippet actions()}
    <button type="button" class="btn btn-danger" onclick={() => void begin({ kind: "uninstall", stock_kernel: null, force_stock_kernel: false })}>
      Return the console to its original system
    </button>
    <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (confirmUninstall = false)}>Keep hakchi</button>
  {/snippet}
</Dialog>

<style>
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow: hidden;
  }

  /* Narrow: the stages and the progress stack in one scrolling column. Wide (1100 px and
     up): two columns, the stage cards on the left and the step list, the run log and the
     result on the right; the step list and the log scroll inside their column. */
  .columns {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    overflow: auto;
    max-width: 1024px;
  }

  .right {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .progress {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-5);
  }

  .progress-head {
    flex: none;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .progress-head .muted {
    font-size: var(--text-md);
  }

  .tools {
    flex: none;
    padding: var(--space-3) var(--space-5);
  }

  .tools h2 {
    margin-bottom: var(--space-1);
    font-size: var(--text-md);
  }

  .tools-list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
  }

  .tools-list li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-1) 0;
  }

  .tool-text {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: var(--space-2);
    align-items: baseline;
    font-size: var(--text-sm);
    white-space: nowrap;
    overflow: hidden;
  }

  .tool-text .muted {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tools-list .btn {
    flex: none;
    width: 104px;
    justify-content: center;
    min-height: var(--control-sm, 28px);
    padding-top: 2px;
    padding-bottom: 2px;
  }

  .tools-list .btn {
    flex: none;
  }

  .steps-scroll {
    min-height: 0;
    overflow: auto;
  }

  @media (min-width: 1100px) {
    .columns {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(380px, 500px);
      grid-template-rows: minmax(0, 1fr);
      align-items: start;
      gap: var(--space-5);
      max-width: 1480px;
      overflow: hidden;
    }

    .left {
      height: 100%;
      overflow: auto;
    }

    /* The last stage grows so the left column ends where the right one does. */
    .left > .stage:last-child {
      flex: 1;
    }

    .left > .stage:last-child .body,
    .left > .stage:last-child .result {
      flex: 1;
    }

    .right {
      max-height: 100%;
      min-height: 0;
    }

    .progress {
      flex: 1 1 auto;
      min-height: 0;
    }

    .steps-scroll {
      flex: 0 1 auto;
    }

  }

  .stages {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .stage {
    position: relative;
    display: grid;
    grid-template-columns: 36px 1fr;
    gap: var(--space-4);
  }

  .stage::before {
    content: "";
    position: absolute;
    left: 17px;
    top: 40px;
    bottom: calc(-1 * var(--space-4));
    width: 2px;
    background: var(--border);
  }

  .stage:last-child::before {
    display: none;
  }

  .number {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-weight: var(--weight-bold);
    font-variant-numeric: tabular-nums;
    transition:
      background-color var(--motion) var(--ease-out),
      border-color var(--motion) var(--ease-out),
      color var(--motion) var(--ease-out);
  }

  .stage.complete .number {
    background: var(--ok-soft);
    border-color: var(--ok);
    color: var(--ok);
  }

  .stage.active .number {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent-strong);
  }

  .stage.failed .number {
    background: var(--danger-soft);
    border-color: var(--danger);
    color: var(--danger);
  }

  .body {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .body.card,
  .result {
    padding: var(--space-4) var(--space-5);
  }

  .summary {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--surface-raised);
    font-size: var(--text-md);
  }

  .summary-text {
    flex: 1;
    min-width: 0;
  }

  .result-actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
    margin-top: var(--space-3);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--control);
  }

  .spacer {
    flex: 1;
  }

  .fel {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-5);
    align-items: center;
  }

  .howto {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .watch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control);
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--surface-raised);
  }

  .version {
    font-size: var(--text-lg);
  }

  .small {
    font-size: var(--text-md);
  }

  .problem {
    color: var(--danger);
    font-size: var(--text-md);
  }

  .elapsed {
    font-size: var(--text-md);
  }

  .note {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--text);
    font-size: var(--text-md);
  }

  .result h2 {
    margin-bottom: var(--space-2);
  }

  .result > div {
    margin-top: var(--space-3);
  }

  .placeholder {
    border-style: dashed;
    box-shadow: none;
  }

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }

  /* The illustration sits beside the steps only when the stage card is wide enough for
     both: a narrow window, or the left column of a two-column window under 1500 px. */
  @media (max-width: 1023px), (min-width: 1100px) and (max-width: 1499px) {
    .fel {
      grid-template-columns: 1fr;
    }
  }
</style>
