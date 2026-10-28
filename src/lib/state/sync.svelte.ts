import { syncPlan, syncRun, syncStatus } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { SyncPlanView, SyncSnapshot } from "../api/types";
import { previewSync } from "../dev/preview";
import { flowStore } from "./flow.svelte";
import { libraryStore } from "./library.svelte";
import { toasts } from "./toasts.svelte";

const TICK_MS = 250;
const PLAN_DELAY_MS = 400;

/** Before any sync. */
export function idleSync(): SyncSnapshot {
  return {
    seq: 0,
    state: "idle",
    plan: null,
    step: null,
    upload: null,
    log: [],
    report: null,
    error: null,
    started_at: null,
    finished_at: null,
  };
}

class SyncStore {
  plan = $state<SyncPlanView | null>(null);
  planning = $state(false);
  planError = $state<string | null>(null);
  snapshot = $state<SyncSnapshot>(idleSync());
  /** The plan is open for review before the sync starts. */
  reviewing = $state(false);
  starting = $state(false);
  now = $state(Date.now());
  private started = false;
  private timer: ReturnType<typeof setInterval> | null = null;
  private planTimer: ReturnType<typeof setTimeout> | null = null;
  /** Whether the tree was reloaded for this console's built-in games (entries and art). */
  private originalsLoaded = false;

  get running(): boolean {
    return this.snapshot.state === "running";
  }

  /** Subscribes to sync events and reads the current snapshot. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      const fixture = previewSync();
      this.plan = fixture.plan;
      if (fixture.snapshot) {
        this.set(fixture.snapshot);
      }
      this.reviewing = fixture.reviewing;
      return;
    }
    await on<SyncSnapshot>("sync-progress", (snapshot) => this.set(snapshot));
    try {
      this.set(await syncStatus());
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the sync status: ${e.message}`, e.detail);
    }
  }

  /** Plans soon, once the changes settle. */
  schedulePlan(): void {
    if (!inTauri) {
      return;
    }
    if (this.planTimer) {
      clearTimeout(this.planTimer);
    }
    this.planTimer = setTimeout(() => {
      this.planTimer = null;
      void this.makePlan();
    }, PLAN_DELAY_MS);
  }

  /** Forgets the plan (the console went away). */
  invalidate(): void {
    if (this.planTimer) {
      clearTimeout(this.planTimer);
      this.planTimer = null;
    }
    this.plan = null;
    this.planError = null;
    this.reviewing = false;
    this.originalsLoaded = false;
  }

  /** Asks the console what a sync would do. */
  async makePlan(): Promise<SyncPlanView | null> {
    if (!inTauri || this.planning || this.running) {
      return this.plan;
    }
    this.planning = true;
    try {
      this.plan = await syncPlan();
      this.planError = null;
      // The first plan against a console caches its built-in games and their art: show
      // them in the tree. Once per connection, so a console without art is not polled.
      const originals = libraryStore.menu?.originals ?? [];
      if (
        this.plan.originals > 0 &&
        !this.originalsLoaded &&
        (originals.length === 0 || originals.some((o) => !o.cover))
      ) {
        this.originalsLoaded = true;
        void libraryStore.load();
      }
    } catch (error: unknown) {
      this.plan = null;
      this.planError = AppError.from(error).message;
    } finally {
      this.planning = false;
    }
    return this.plan;
  }

  /** Opens the plan for review, refreshing it first. */
  async review(): Promise<void> {
    if (this.running) {
      return;
    }
    await this.makePlan();
    if (this.plan) {
      this.reviewing = true;
    } else if (this.planError) {
      toasts.error(this.planError);
    }
  }

  cancelReview(): void {
    this.reviewing = false;
  }

  /** Runs the sync. */
  async run(): Promise<void> {
    if (this.starting || this.running) {
      return;
    }
    if (!inTauri) {
      toasts.info("Syncing works inside the desktop app only.");
      return;
    }
    this.reviewing = false;
    this.starting = true;
    try {
      const plan = this.plan ?? (await this.makePlan());
      if (plan?.needs_7z && !(await this.add7z())) {
        return;
      }
      const snapshot = await syncRun();
      this.set(snapshot);
      if (snapshot.state === "done" && snapshot.report) {
        const r = snapshot.report;
        toasts.success(
          `Synced: ${r.uploaded_files} file${r.uploaded_files === 1 ? "" : "s"} uploaded, ${r.removed_files} removed.`,
        );
      } else if (snapshot.error) {
        toasts.error(snapshot.error.message, snapshot.error.detail);
      }
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.starting = false;
      this.schedulePlan();
    }
  }

  /** Adds 7z support to the console (the `base_mods` flow, one restart), then waits until
      the console answers again. False when the flow did not finish. */
  private async add7z(): Promise<boolean> {
    if (!(await flowStore.begin({ kind: "base_mods" }))) {
      return false;
    }
    const done = await flowStore.settled();
    if (done.state !== "done") {
      return false;
    }
    // The watcher reconnects after the flow; the sync needs its session.
    for (let attempt = 0; attempt < 45; attempt++) {
      this.plan = null;
      if (await this.makePlan()) {
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, 2000));
    }
    toasts.error("7z support was added, but the console did not answer again. Try the sync once more.");
    return false;
  }

  /** Puts the result away; the bar shows the plan again. */
  dismiss(): void {
    if (!this.running) {
      this.set({ ...idleSync(), seq: this.snapshot.seq + 1 });
    }
  }

  private set(snapshot: SyncSnapshot): void {
    if (snapshot.seq < this.snapshot.seq) {
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

export const syncStore = new SyncStore();
