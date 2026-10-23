import { modulesFetch, modulesStatus } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { ModuleDownload, ModulesStatus } from "../api/types";
import { previewModules } from "../dev/preview";
import { flowStore } from "./flow.svelte";
import { navigation } from "./navigation.svelte";
import { toasts } from "./toasts.svelte";

/** Which modules a need translates to: RetroArch plus the core. */
export const NES_MODULES = ["retroarch", "fceumm"];
export const SNES_MODULES = ["retroarch", "snes9x"];

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

  /** Downloads what is missing from the cache, then starts the install flow for `ids` and
   *  shows the Kernel page, where the flow's steps are followed. */
  async install(ids: string[]): Promise<boolean> {
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
