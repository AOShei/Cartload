import { modulesFetch, modulesStatus } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { ModuleDownload, ModulesStatus } from "../api/types";
import { previewModules } from "../dev/preview";
import { flowStore } from "./flow.svelte";
import { navigation } from "./navigation.svelte";
import { toasts } from "./toasts.svelte";

/** The modules a core needs on the console: RetroArch plus the core. */
export function withRetroarch(core: string): string[] {
  return core === "retroarch" ? ["retroarch"] : ["retroarch", core];
}

/** An install waiting for the user to accept a module's warning. */
export interface PendingInstall {
  ids: string[];
  warnings: { title: string; text: string }[];
}

/* Emulator modules from the Mod Hub: the catalog with its cache and console state, downloads
   with progress, and the hand-off to the install flow (which runs on the Kernel page like
   every flow). */
class ModulesStore {
  status = $state<ModulesStatus | null>(null);
  loading = $state(false);
  /** Ids being downloaded right now. */
  downloading = $state<string[]>([]);
  /** Download progress per module id. */
  progress = $state<Record<string, ModuleDownload>>({});
  /** An install that asks for confirmation first (a module with a warning). */
  pending = $state<PendingInstall | null>(null);
  private started = false;

  /** The modules with this id that are not yet on the console (unknown counts as missing). */
  missing(ids: string[]): string[] {
    const modules = this.status?.modules ?? [];
    return ids.filter((id) => modules.find((m) => m.id === id)?.installed !== true);
  }

  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      this.status = previewModules();
      return;
    }
    await on<ModuleDownload>("module-download", (progress) => {
      this.progress = { ...this.progress, [progress.id]: progress };
    });
    await this.load();
  }

  /** Reads the catalog state again (cache, and the console when one is connected). */
  async load(): Promise<void> {
    if (!inTauri || this.loading) {
      return;
    }
    this.loading = true;
    try {
      this.status = await modulesStatus();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the emulator modules: ${e.message}`, e.detail);
    } finally {
      this.loading = false;
    }
  }

  /** Installs `ids`, asking first when one of them carries a warning (`pending`, shown by
   *  `ModuleWarningDialog`). */
  async install(ids: string[]): Promise<boolean> {
    const warnings = ids
      .map((id) => this.status?.modules.find((m) => m.id === id))
      .filter((m) => m?.warning)
      .map((m) => ({ title: m!.title, text: m!.warning! }));
    if (warnings.length > 0) {
      this.pending = { ids, warnings };
      return false;
    }
    return this.installNow(ids);
  }

  /** Accepts the pending install's warnings and installs. */
  async confirmPending(): Promise<boolean> {
    const pending = this.pending;
    this.pending = null;
    return pending ? this.installNow(pending.ids) : false;
  }

  /** Downloads what is missing from the cache, then starts the install flow for `ids` and
   *  shows the Kernel page, where the flow's steps are followed. */
  private async installNow(ids: string[]): Promise<boolean> {
    if (!inTauri) {
      toasts.info("Installing modules works inside the desktop app only.");
      return false;
    }
    if (this.downloading.length > 0 || flowStore.running) {
      toasts.info("Another download or flow is in progress.");
      return false;
    }
    this.downloading = ids;
    this.progress = {};
    try {
      this.status = await modulesFetch(ids);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      return false;
    } finally {
      this.downloading = [];
    }
    const started = await flowStore.begin({ kind: "install_modules", ids });
    if (started) {
      navigation.go("kernel");
      toasts.info("Installing the emulator modules. The console reboots once at the end.");
    }
    return started;
  }
}

export const modulesStore = new ModulesStore();
