import { flowCancel, flowStart, flowStatus, flowSteps } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { FlowKind, FlowSnapshot, StepSnapshot } from "../api/types";
import { previewFlow } from "../dev/preview";
import { toasts } from "./toasts.svelte";

/** The guided install's stages (not the flow's steps). */
export type Stage = "fel" | "package" | "run" | "result";

const TICK_MS = 250;

/** An empty snapshot, before any flow ran. */
export function idleSnapshot(): FlowSnapshot {
  return {
    seq: 0,
    kind: null,
    state: "idle",
    steps: [],
    current: null,
    message: null,
    failure: null,
    cancel_pending: false,
    started_at: null,
    finished_at: null,
  };
}

/** What a flow does, as a title. */
export function flowTitle(kind: FlowKind | null): string {
  switch (kind?.kind) {
    case "memboot":
      return "Boot from memory";
    case "uninstall":
      return "Return to the original system";
    case "dump_kernel":
      return "Save the stock kernel";
    case "install":
    default:
      return "Install hakchi";
  }
}

/** `0.4 s`, `12 s`, `2 min 05 s`. */
export function formatDuration(ms: number): string {
  const secs = Math.max(0, ms) / 1000;
  if (secs >= 60) {
    const minutes = Math.floor(secs / 60);
    const rest = Math.round(secs % 60);
    return `${minutes} min ${rest.toString().padStart(2, "0")} s`;
  }
  if (secs >= 10) {
    return `${Math.round(secs)} s`;
  }
  return `${secs.toFixed(1)} s`;
}

class FlowStore {
  snapshot = $state<FlowSnapshot>(idleSnapshot());
  /** Steps of the install flow, for the list before anything runs. */
  installSteps = $state<StepSnapshot[]>([]);
  /** Updated a few times a second while a flow runs, for live timings. */
  now = $state(Date.now());
  starting = $state(false);
  cancelling = $state(false);
  private started = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  get running(): boolean {
    return this.snapshot.state === "running";
  }

  get kind(): FlowKind | null {
    return this.snapshot.kind;
  }

  /** The steps to show: the running or last flow's, else the install flow's, pending. */
  get steps(): StepSnapshot[] {
    return this.snapshot.state === "idle" ? this.installSteps : this.snapshot.steps;
  }

  /** Subscribes to flow events and fetches the current snapshot. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      const fixture = previewFlow();
      if (fixture) {
        this.set(fixture);
      }
      this.installSteps = fixture?.steps ?? [];
      return;
    }
    await on<FlowSnapshot>("flow-progress", (snapshot) => this.set(snapshot));
    try {
      this.set(await flowStatus());
      this.installSteps = await flowSteps({ kind: "install" });
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the flow status: ${e.message}`, e.detail);
    }
  }

  /** Starts a flow. Returns whether it started. */
  async begin(kind: FlowKind): Promise<boolean> {
    if (this.starting || this.running) {
      return false;
    }
    if (!inTauri) {
      toasts.info("Flows run inside the desktop app only.");
      return false;
    }
    this.starting = true;
    try {
      this.set(await flowStart(kind));
      return true;
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      return false;
    } finally {
      this.starting = false;
    }
  }

  /** Asks the running flow to stop. */
  async cancel(): Promise<void> {
    if (this.cancelling || !this.running) {
      return;
    }
    this.cancelling = true;
    try {
      this.set(await flowCancel());
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.cancelling = false;
    }
  }

  private set(snapshot: FlowSnapshot): void {
    // Events and command replies can cross; keep the newest.
    if (snapshot.seq < this.snapshot.seq && snapshot.state !== "idle") {
      return;
    }
    this.snapshot = snapshot;
    this.now = Date.now();
    if (snapshot.state === "running") {
      this.timer ??= setInterval(() => {
        this.now = Date.now();
      }, TICK_MS);
    } else if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }
}

export const flowStore = new FlowStore();
