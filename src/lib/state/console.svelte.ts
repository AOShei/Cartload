import { consoleInfo, consoleStatus, felProbe } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { FelCheck, StatusReport } from "../api/types";
import { previewReport } from "../dev/preview";
import { toasts } from "./toasts.svelte";

class ConsoleStore {
  report = $state<StatusReport | null>(null);
  refreshing = $state(false);
  probing = $state(false);
  private started = false;

  /** Subscribes to status changes and fetches the current one. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      this.report = previewReport();
      return;
    }
    await on<StatusReport>("console-status", (report) => {
      this.report = report;
    });
    try {
      this.report = await consoleStatus();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the console status: ${e.message}`, e.detail);
    }
  }

  /** Looks again now; re-reads the details of a connected console. */
  async refresh(): Promise<void> {
    if (this.refreshing) {
      return;
    }
    this.refreshing = true;
    try {
      this.report = await consoleInfo();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Refresh failed: ${e.message}`, e.detail);
    } finally {
      this.refreshing = false;
    }
  }

  /** Asks the attached FEL device for its version block. */
  async probeFel(): Promise<void> {
    if (this.probing) {
      return;
    }
    this.probing = true;
    try {
      const probe = await felProbe();
      toasts.info(describeFelCheck(probe.check));
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.probing = false;
    }
  }
}

/** One sentence for a FEL probe result. */
export function describeFelCheck(check: FelCheck): string {
  switch (check.kind) {
    case "fel":
      return check.r16
        ? `FEL mode confirmed: ${check.soc}, the chip of the Classic Mini consoles.`
        : `A FEL device answered, but its chip is ${check.soc}, not the consoles' Allwinner R16.`;
    case "not_fel":
      return `The device did not answer as the FEL boot ROM: ${check.reason}`;
    case "unavailable":
      return `The FEL device could not be opened: ${check.reason}`;
    case "unchecked":
      return "The FEL device has not been checked yet.";
  }
}

export const consoleStore = new ConsoleStore();
