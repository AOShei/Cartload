import { logRecent } from "../api/commands";
import { inTauri, on, AppError } from "../api/tauri";
import type { LogLine } from "../api/types";
import { preferences } from "./preferences.svelte";
import { toasts } from "./toasts.svelte";

/** Lines kept in memory; older ones are dropped. */
export const LOG_CAP = 500;

class LogStore {
  lines = $state<LogLine[]>([]);
  private lastSeq = 0;
  private started = false;

  /** Lines to show, honouring the debug preference. */
  get visible(): LogLine[] {
    if (preferences.showDebugLog) {
      return this.lines;
    }
    return this.lines.filter((line) => line.level !== "debug" && line.level !== "trace");
  }

  /** Subscribes to live lines and loads the backlog. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      this.add({
        seq: 1,
        time: Date.now(),
        level: "info",
        source: "app",
        message: "Running outside the desktop shell: console detection is unavailable.",
      });
      return;
    }
    // Live lines that arrive while the backlog loads are held back, then merged by seq.
    const pending: LogLine[] = [];
    let loading = true;
    await on<LogLine>("log", (line) => {
      if (loading) {
        pending.push(line);
      } else {
        this.add(line);
      }
    });
    try {
      const recent = await logRecent();
      for (const line of recent) {
        this.add(line);
      }
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not load the activity log: ${e.message}`, e.detail);
    } finally {
      loading = false;
      pending.sort((a, b) => a.seq - b.seq);
      for (const line of pending) {
        this.add(line);
      }
    }
  }

  /** Appends a line (ignoring any already seen) and raises a toast for errors. */
  add(line: LogLine): void {
    if (line.seq <= this.lastSeq) {
      return;
    }
    this.lastSeq = line.seq;
    this.lines.push(line);
    if (this.lines.length > LOG_CAP) {
      this.lines.splice(0, this.lines.length - LOG_CAP);
    }
    if (line.level === "error") {
      toasts.error(line.message);
    }
  }

  clear(): void {
    this.lines = [];
  }

  /** The visible lines as text, for copying. */
  asText(): string {
    return this.visible
      .map((line) => `${formatTime(line.time)} ${line.level.padEnd(5)} ${line.source}: ${line.message}`)
      .join("\n");
  }
}

/** `HH:MM:SS` in local time. */
export function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour12: false });
}

export const logStore = new LogStore();
