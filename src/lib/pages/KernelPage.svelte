<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { inTauri } from "../api/tauri";
  import type { FlowKind } from "../api/types";
  import DetailsList from "../components/DetailsList.svelte";
  import FailureCard from "../components/FailureCard.svelte";
  import FelIllustration from "../components/FelIllustration.svelte";
  import FlowLog from "../components/FlowLog.svelte";
  import Icon from "../components/Icon.svelte";
  import StatusCard from "../components/StatusCard.svelte";
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
      text: "Waiting for the console… Nothing found on USB yet.",
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
              text: "Console attached, but in normal boot. Switch it off and do the Reset trick above.",
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
            text: "Console running hakchi from memory. Ready; no Reset needed.",
            ready: true,
            installed: false,
          };
        }
        return {
          tone: "ready",
          text: `hakchi ${status.info.versions?.script ?? ""} is installed. Installing again updates or repairs it from the running kernel; no Reset needed.`,
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

  const resultPrimary = $derived.by(() => {
    if (snapshot.state !== "done") {
      return null;
    }
    if (snapshot.kind?.kind === "install" && status?.state === "connected") {
      return { label: "Open Library", run: () => navigation.go("library") };
    }
    return { label: "Open Console", run: () => navigation.go("console") };
  });

  let menuOpen = $state(false);
  let menu: HTMLDetailsElement | undefined = $state();
  let confirmUninstall = $state(false);

  /** The More menu closes on a click elsewhere or on Escape, like a native menu. */
  function onwindowclick(event: MouseEvent): void {
    if (menuOpen && menu && !menu.contains(event.target as Node)) {
      menuOpen = false;
    }
  }

  function onwindowkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      if (menuOpen) {
        menuOpen = false;
      }
      confirmUninstall = false;
    }
  }

  async function begin(kind: FlowKind): Promise<void> {
    menuOpen = false;
    confirmUninstall = false;
    await flowStore.begin(kind);
  }

  function retry(): void {
    const kind = snapshot.kind ?? { kind: "install" };
    void begin(kind);
  }

  async function dumpKernel(): Promise<void> {
    menuOpen = false;
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

<svelte:window onclick={onwindowclick} onkeydown={onwindowkeydown} />

<div class="page">
  <header class="top">
    <p class="lede">
      Four stages: put the console in FEL mode, check the kernel package, run the install, and
      confirm the result. Nothing is written until stage 3 starts.
    </p>
    <details class="menu" bind:open={menuOpen} bind:this={menu}>
      <summary class="btn btn-quiet" aria-haspopup="menu">
        More
        <Icon name="more" size={16} />
      </summary>
      <div class="menu-items" role="menu" aria-label="Other actions">
        <button type="button" role="menuitem" disabled={!canStart} onclick={() => void begin({ kind: "memboot" })}>
          <Icon name="play" size={16} />
          <span>Boot from memory only <small>Recovery shell, nothing written</small></span>
        </button>
        <button type="button" role="menuitem" disabled={!canStart} onclick={() => { menuOpen = false; confirmUninstall = true; }}>
          <Icon name="undo" size={16} />
          <span>Return to the original system… <small>Removes hakchi</small></span>
        </button>
        <button type="button" role="menuitem" disabled={!canStart} onclick={() => void dumpKernel()}>
          <Icon name="save" size={16} />
          <span>Save the stock kernel… <small>Backup to a file</small></span>
        </button>
      </div>
    </details>
  </header>

  <ol class="stages">
    <!-- Stage 1 -->
    <li class="stage" class:complete={watch.ready}>
      <span class="number" aria-hidden="true">{watch.ready ? "✓" : "1"}</span>
      <div class="body card">
        <h2>Put the console in FEL mode</h2>
        <div class="fel">
          <FelIllustration width={300} />
          <ol class="howto">
            <li>Connect the console to this computer with its USB cable and switch POWER off.</li>
            <li>Hold RESET down and switch POWER on.</li>
            <li>Keep RESET held for two seconds, then release it. The screen stays black: that is FEL mode.</li>
          </ol>
        </div>
        <p class="watch tone-{watch.tone}" role="status">
          <span class="dot" aria-hidden="true"></span>
          {watch.text}
        </p>
      </div>
    </li>

    <!-- Stage 2 -->
    <li class="stage" class:complete={pkg !== null}>
      <span class="number" aria-hidden="true">{pkg ? "✓" : "2"}</span>
      <div class="body card">
        <div class="row">
          <h2>Kernel package</h2>
          <span class="spacer"></span>
          {#if packageStore.updating}
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
        {:else if packageStore.updating}
          <p class="muted">Fetching the latest hakchi.hmod…</p>
          <progress max={packageStore.progress?.total ?? 1} value={packageStore.progress?.total ? packageStore.progress.done : undefined}></progress>
        {:else}
          <p>No kernel package yet. It is a 9 MB download from the hakchi project's servers.</p>
          {#if packageStore.status?.problem}
            <p class="problem">The copy on disk cannot be used: {packageStore.status.problem}</p>
          {/if}
          <div>
            <button type="button" class="btn btn-primary" onclick={() => void packageStore.update()} disabled={busy}>
              <Icon name="download" size={16} />
              Download package
            </button>
          </div>
        {/if}
      </div>
    </li>

    <!-- Stage 3 -->
    <li class="stage" class:complete={snapshot.state === "done"} class:active={running}>
      <span class="number" aria-hidden="true">{snapshot.state === "done" ? "✓" : "3"}</span>
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
              {snapshot.cancel_pending ? "Stopping after this step…" : "Stop"}
            </button>
          {:else if snapshot.state === "idle"}
            <button type="button" class="btn btn-primary" onclick={() => void begin({ kind: "install" })} disabled={!canStart}>
              {watch.installed ? "Reinstall hakchi" : "Install hakchi"}
              <Icon name="arrow" size={16} />
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
        {#if confirmUninstall}
          <div class="confirm" role="group" aria-label="Confirm returning to the original system">
            <p>
              This flashes the original kernel back and removes hakchi from the console. Games
              added with hakchi will no longer appear; the console's own games are untouched.
            </p>
            <div class="confirm-actions">
              <button type="button" class="btn danger" onclick={() => void begin({ kind: "uninstall", stock_kernel: null, force_stock_kernel: false })}>
                Return the console to its original system
              </button>
              <button type="button" class="btn btn-quiet" onclick={() => (confirmUninstall = false)}>Keep hakchi</button>
            </div>
          </div>
        {/if}
        {#if snapshot.cancel_pending}
          <p class="note">The current step writes to the console's flash, so it finishes first. Do not unplug the console.</p>
        {/if}
        <StepList steps={flowStore.steps} now={flowStore.now} />
        {#if snapshot.state !== "idle"}
          <FlowLog since={snapshot.started_at} />
        {/if}
      </div>
    </li>

    <!-- Stage 4 -->
    <li class="stage" class:complete={snapshot.state === "done"} class:failed={snapshot.state === "failed"}>
      <span class="number" aria-hidden="true">{snapshot.state === "done" ? "✓" : "4"}</span>
      <div class="body">
        {#if outcome}
          <section class="card result">
            <h2>{outcome.heading}</h2>
            <p class="muted">{outcome.text}</p>
          </section>
          <StatusCard
            {report}
            refreshing={consoleStore.refreshing}
            primary={resultPrimary}
            onrefresh={() => void consoleStore.refresh()}
          />
          {#if report && report.details.length > 0}
            <section class="card details" aria-label="Detected details">
              <DetailsList details={report.details} />
            </section>
          {/if}
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
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 64rem;
  }

  .top {
    display: flex;
    align-items: flex-start;
    gap: var(--space-4);
  }

  .lede {
    color: var(--text-muted);
    max-width: 60ch;
  }

  .menu {
    position: relative;
    margin-left: auto;
    flex: none;
  }

  .menu > summary {
    list-style: none;
  }

  .menu > summary::-webkit-details-marker {
    display: none;
  }

  .menu-items {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 10;
    min-width: 20rem;
    padding: var(--space-1);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    box-shadow: var(--shadow-float);
    display: flex;
    flex-direction: column;
  }

  .menu-items button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .menu-items button:hover:not(:disabled) {
    background: var(--surface-hover);
  }

  .menu-items button:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .menu-items small {
    display: block;
    color: var(--text-muted);
    font-size: 0.8rem;
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
    grid-template-columns: 2.25rem 1fr;
    gap: var(--space-4);
  }

  .stage::before {
    content: "";
    position: absolute;
    left: calc(1.125rem - 1px);
    top: 2.5rem;
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
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 50%;
    background: var(--surface-raised);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
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
  .result,
  .details {
    padding: var(--space-4) var(--space-5);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 2.25rem;
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
    padding-left: 1.2rem;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 48ch;
  }

  .watch {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--surface-raised);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-faint);
    flex: none;
  }

  .tone-ready .dot {
    background: var(--ok);
  }

  .tone-busy .dot {
    background: var(--accent);
    animation: blink 1.4s ease-in-out infinite;
  }

  .tone-warning .dot {
    background: var(--warn);
  }

  .tone-error .dot {
    background: var(--danger);
  }

  .version {
    font-size: 1.1rem;
  }

  .small {
    font-size: 0.85rem;
  }

  .problem {
    color: var(--danger);
    font-size: 0.9rem;
  }

  .elapsed {
    font-size: 0.9rem;
  }

  .note {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--text);
    font-size: 0.9rem;
  }

  .confirm {
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--danger);
    border-radius: var(--radius-m);
    background: var(--danger-soft);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .confirm-actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
    font-weight: 600;
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

  progress {
    width: 100%;
    height: 6px;
    accent-color: var(--accent);
  }

  @keyframes blink {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 1;
    }
  }

  @media (max-width: 900px) {
    .fel {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tone-busy .dot {
      animation: none;
    }
  }
</style>
