import { flowCancel, flowStart, flowStatus, flowSteps } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { FlowKind, FlowSnapshot, StepSnapshot } from "../api/types";
import { previewFlow } from "../dev/preview";
import { navigation } from "./navigation.svelte";
import { preferences } from "./preferences.svelte";
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
      return "Factory reset";
    case "dump_kernel":
      return "Save the stock kernel";
    case "install_modules":
      return "Install emulators";
    case "update_modules":
      return "Update emulators";
    case "remove_modules":
      return "Remove emulators";
    case "base_mods":
      return "Add 7z support";
    case "install":
    default:
      return "Install the system";
  }
}

/** Whether a flow may boot the console through FEL, which needs the user at the console. */
export function touchesKernel(kind: FlowKind | null): boolean {
  return (
    kind?.kind !== "install_modules" &&
    kind?.kind !== "update_modules" &&
    kind?.kind !== "remove_modules" &&
    kind?.kind !== "base_mods"
  );
}

/** What a finished flow did, as a heading and one sentence. */
export function flowOutcome(kind: FlowKind | null): { heading: string; text: string } {
  switch (kind?.kind) {
    case "memboot":
      return {
        heading: "The console runs the custom system from memory.",
        text: "Nothing was written to it. Switch it off and on to return to its own system.",
      };
    case "uninstall":
      return {
        heading: "The console is back on its original system.",
        text: "The custom system was removed and the original kernel flashed back. The console boots its own menu now.",
      };
    case "dump_kernel":
      return {
        heading: "The stock kernel was saved.",
        text: `Written to ${kind.path}. Keep it: it returns the console to its original system if the backup on the console is ever lost.`,
      };
    case "install_modules":
      return {
        heading: "The emulators are installed.",
        text: "The console rebooted with them. Games that need them start from its menu after the next sync.",
      };
    case "update_modules":
      return {
        heading: "The emulators are updated.",
        text: "The console rebooted with the new versions. Your games and their settings are unchanged.",
      };
    case "remove_modules":
      return {
        heading: "The emulators are removed.",
        text: "The console rebooted without them. The built-in emulators and your other cores are untouched.",
      };
    case "base_mods":
      return {
        heading: "7z support is added.",
        text: "The console restarted with it. Compressed games start from its menu after the sync.",
      };
    case "install":
    default:
      return {
        heading: "The custom system is installed.",
        text: "The console rebooted into its menu with the custom system running.",
      };
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
  /** The progress window is showing. Closing it hides it; the flow keeps running. */
  windowOpen = $state(false);
  /** Sequence number of the finished run the user put away, so the status bar stops
   *  pointing at it. */
  dismissedSeq = $state(-1);
  /** The run was started by another view that shows its progress (the sync bar), so it
   *  opens no window and its success is not announced. */
  private quiet = false;
  private started = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  get running(): boolean {
    return this.snapshot.state === "running";
  }

  get kind(): FlowKind | null {
    return this.snapshot.kind;
  }

  /** A finished run the status bar still points at: failed or stopped, not yet put away. */
  get unresolved(): boolean {
    const state = this.snapshot.state;
    return (state === "failed" || state === "cancelled") && this.snapshot.seq !== this.dismissedSeq;
  }

  showWindow(): void {
    this.windowOpen = true;
  }

  /** Hides the window. A finished run is put away with it; a running one carries on. */
  hideWindow(): void {
    this.windowOpen = false;
    if (this.snapshot.state === "done") {
      this.dismissedSeq = this.snapshot.seq;
    }
  }

  /** Forgets a failed or stopped run (the status bar's ×). */
  dismiss(): void {
    this.dismissedSeq = this.snapshot.seq;
    this.windowOpen = false;
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
      // `?window=open` shows the progress window over the page.
      this.windowOpen = new URLSearchParams(location.search).get("window") === "open";
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

  /** Starts a flow and opens its window, unless `window` is false because the caller shows
   *  the progress itself. Returns whether it started. */
  async begin(kind: FlowKind, { window = true }: { window?: boolean } = {}): Promise<boolean> {
    if (this.starting || this.running) {
      return false;
    }
    if (!inTauri) {
      toasts.info("Flows run inside the desktop app only.");
      return false;
    }
    this.starting = true;
    try {
      this.quiet = !window;
      this.set(await flowStart(kind, preferences.kernelPackage));
      this.windowOpen = window;
      return true;
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      return false;
    } finally {
      this.starting = false;
    }
  }

  /** Resolves with the final snapshot once no flow is starting or running. */
  async settled(): Promise<FlowSnapshot> {
    while (this.starting || this.running) {
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    return this.snapshot;
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

  /** A run ended while its window was hidden: say so, with a way back to it. */
  private announce(snapshot: FlowSnapshot): void {
    const title = flowTitle(snapshot.kind);
    const show = { label: "Show", run: () => this.showWindow() };
    if (snapshot.state === "done") {
      this.dismissedSeq = snapshot.seq;
      if (this.quiet) {
        return;
      }
      toasts.withAction("success", flowOutcome(snapshot.kind).heading, {
        label: "Open Console",
        run: () => navigation.go("console"),
      });
    } else if (snapshot.state === "failed") {
      toasts.withAction("error", `${title} failed: ${snapshot.failure?.title ?? "see the details"}`, show);
    } else if (snapshot.state === "cancelled") {
      toasts.withAction("info", `${title} stopped.`, show);
    }
  }

  private set(snapshot: FlowSnapshot): void {
    // Events and command replies can cross; keep the newest.
    if (snapshot.seq < this.snapshot.seq && snapshot.state !== "idle") {
      return;
    }
    const finished = this.snapshot.state === "running" && snapshot.state !== "running";
    this.snapshot = snapshot;
    if (finished && !this.windowOpen) {
      this.announce(snapshot);
    }
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
