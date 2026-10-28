<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { inTauri } from "../api/tauri";
  import type { FlowKind } from "../api/types";
  import {
    consoleStore,
    flowOutcome,
    flowStore,
    flowTitle,
    formatDuration,
    modulesStore,
    navigation,
    toasts,
    touchesKernel,
  } from "../state";
  import FelInstructions from "./console/FelInstructions.svelte";
  import Dialog from "./Dialog.svelte";
  import FailureCard from "./FailureCard.svelte";
  import FlowLog from "./FlowLog.svelte";
  import Icon from "./Icon.svelte";
  import StepList from "./StepList.svelte";

  /* The progress of the task running on the console, over whatever page started it: the
     steps, the log, and what each task needs from the user (the Reset dance while a kernel
     task waits for FEL mode). Closing it hides it; the task keeps running and the status
     bar brings it back. */
  const snapshot = $derived(flowStore.snapshot);
  const kind = $derived(snapshot.kind);
  const running = $derived(flowStore.running);
  const busy = $derived(running || flowStore.starting);
  const kernel = $derived(touchesKernel(kind));

  /** The modules a module task works on, by name. */
  const modules = $derived.by((): string => {
    if (kind?.kind !== "install_modules" && kind?.kind !== "update_modules" && kind?.kind !== "remove_modules") {
      return "";
    }
    const catalog = modulesStore.status?.modules ?? [];
    return kind.ids.map((id) => catalog.find((m) => m.id === id)?.title ?? id).join(", ");
  });

  const elapsed = $derived.by((): string => {
    if (snapshot.started_at === null) {
      return "";
    }
    return formatDuration((snapshot.finished_at ?? flowStore.now) - snapshot.started_at);
  });

  const current = $derived(snapshot.current === null ? null : (snapshot.steps[snapshot.current] ?? null));
  const position = $derived(
    current ? `Step ${current.index + 1} of ${snapshot.steps.length}: ${current.label}` : "",
  );

  /** The memboot step is waiting for the user to put the console in FEL mode. */
  const waitingForFel = $derived(
    running && current?.name === "memboot" && /waiting .*FEL mode/i.test(current.message ?? snapshot.message ?? ""),
  );

  const failedStep = $derived.by((): string | null => {
    const index = snapshot.failure?.step_index;
    return index === null || index === undefined ? null : (snapshot.steps[index]?.label ?? null);
  });

  const outcome = $derived(flowOutcome(kind));
  /** After a first install the Library is the next step. */
  const libraryNext = $derived(snapshot.state === "done" && kind?.kind === "install");

  function retry(): void {
    void flowStore.begin(kind ?? { kind: "install" });
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
      await flowStore.begin({ kind: "uninstall", stock_kernel: picked, force_stock_kernel: false });
    }
  }

  function forceStockKernel(): void {
    const last: FlowKind | null = kind;
    if (last?.kind === "uninstall") {
      void flowStore.begin({ kind: "uninstall", stock_kernel: last.stock_kernel ?? null, force_stock_kernel: true });
    }
  }

  function addGames(): void {
    flowStore.hideWindow();
    navigation.go("library");
  }
</script>

<Dialog open={flowStore.windowOpen && snapshot.state !== "idle"} title={flowTitle(kind)} width="720px" onclose={() => flowStore.hideWindow()}>
  <div class="summary" role="status">
    {#if running}
      <span class="spinner" aria-hidden="true"></span>
      <span class="what">{modules || position || "Starting…"}</span>
    {:else if snapshot.state === "done"}
      <Icon name="check" size={16} />
      <span class="what">Finished{modules ? `: ${modules}` : ""}</span>
    {:else if snapshot.state === "failed"}
      <Icon name="warning" size={16} />
      <span class="what">Failed{failedStep ? ` at ${failedStep}` : ""}</span>
    {:else if snapshot.state === "cancelled"}
      <Icon name="stop" size={16} />
      <span class="what">Stopped</span>
    {/if}
    {#if elapsed}<span class="muted num">{elapsed}</span>{/if}
  </div>
  {#if running && modules && position}
    <p class="muted small step">{position}</p>
  {/if}

  {#if waitingForFel}
    <section class="your-turn" aria-labelledby="fel-heading">
      <h3 id="fel-heading"><Icon name="arrow" size={16} /> Your turn: put the console in FEL mode</h3>
      <FelInstructions width={200} picker={false} />
    </section>
  {/if}

  {#if snapshot.cancel_pending}
    <p class="note">The current step writes to the console's flash, so it finishes first. Do not unplug the console.</p>
  {/if}

  {#if snapshot.state === "done"}
    <section class="result">
      <h3>{outcome.heading}</h3>
      <p class="muted">{outcome.text}</p>
      <p class="console-line">
        <span class="dot tone-{consoleStore.report?.tone ?? 'neutral'}" aria-hidden="true"></span>
        {consoleStore.report?.title ?? "Looking at the console…"}
      </p>
    </section>
  {:else if snapshot.state === "failed" && snapshot.failure}
    <FailureCard
      failure={snapshot.failure}
      stepLabel={failedStep}
      {busy}
      onretry={retry}
      onchoosekernel={() => void chooseStockKernel()}
      onforce={forceStockKernel}
    />
  {:else if snapshot.state === "cancelled"}
    <section class="result">
      <h3>Stopped.</h3>
      <p class="muted">Nothing more was changed. Continue to pick up where it stopped: finished steps are skipped.</p>
    </section>
  {/if}

  <section class="steps" aria-label="Steps">
    <StepList steps={snapshot.steps} now={flowStore.now} />
  </section>

  <details class="log" open={kernel}>
    <summary>Log</summary>
    <FlowLog since={snapshot.started_at} />
  </details>

  {#snippet actions()}
    {#if running}
      <button type="button" class="btn btn-primary" data-autofocus onclick={() => flowStore.hideWindow()}>Hide</button>
      <button type="button" class="btn btn-quiet" onclick={() => void flowStore.cancel()} disabled={flowStore.cancelling || snapshot.cancel_pending}>
        <Icon name="stop" size={16} />
        {snapshot.cancel_pending ? "Stopping after this step…" : flowStore.cancelling ? "Stopping…" : "Stop"}
      </button>
    {:else if snapshot.state === "cancelled"}
      <button type="button" class="btn btn-primary" onclick={retry} disabled={busy}>
        <Icon name="play" size={16} />
        Continue
      </button>
      <button type="button" class="btn btn-quiet" data-autofocus onclick={() => flowStore.dismiss()}>Close</button>
    {:else if snapshot.state === "failed"}
      <button type="button" class="btn btn-quiet" data-autofocus onclick={() => flowStore.dismiss()}>Close</button>
    {:else if libraryNext}
      <button type="button" class="btn btn-primary" data-autofocus onclick={addGames}>Add games</button>
      <button type="button" class="btn btn-quiet" onclick={() => flowStore.hideWindow()}>Close</button>
    {:else}
      <button type="button" class="btn btn-primary" data-autofocus onclick={() => flowStore.hideWindow()}>Close</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .summary {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: var(--weight-semibold);
  }

  .what {
    flex: 1;
    min-width: 0;
  }

  .step {
    margin-top: calc(-1 * var(--space-2));
  }

  .your-turn {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--accent);
    border-radius: var(--radius-m);
    background: var(--accent-soft);
  }

  .your-turn h3 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--accent-strong);
  }

  .result {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-m);
    background: var(--surface-raised);
  }

  .console-line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }

  .steps {
    max-height: 280px;
    overflow: auto;
  }

  .log summary {
    cursor: pointer;
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin-bottom: var(--space-2);
  }

  .note {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    font-size: var(--text-md);
  }

  .small {
    font-size: var(--text-sm);
  }
</style>
