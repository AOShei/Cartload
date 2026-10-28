import { modulesFetch, modulesStatus } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { GameView, ModuleDownload, ModulesStatus } from "../api/types";
import { previewModules } from "../dev/preview";
import { flowStore } from "./flow.svelte";
import { libraryStore } from "./library.svelte";
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

/** A removal the user is asked about: refused while games need the module, else waiting
 *  for confirmation (`ModuleRemoveDialog`). */
export interface PendingRemoval {
  /** The module asked for. */
  id: string;
  title: string;
  /** Installed cores that go with it (removing RetroArch removes them too). */
  alsoRemoved: string[];
  /** Library games that still need the module; the removal waits until there are none. */
  games: GameView[];
  /** Installed without an uninstall record Cartload can run. */
  unmanaged: boolean;
  /** The console's packages the removal uninstalls (`snes9x2010`, `_km_snes9x_xtreme`). */
  records: string[];
}

/* Emulator modules from the Mod Hub: the catalog with its cache and console state, downloads
   with progress, and the hand-off to the install flow (followed in the progress window like
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
  /** A removal being asked about. */
  removal = $state<PendingRemoval | null>(null);
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
      // `?remove=fceumm` opens the removal dialog for that module (after the library loads).
      const wanted = new URLSearchParams(location.search).get("remove");
      if (wanted) {
        void libraryStore.start().then(() => this.requestRemove(wanted));
      }
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

  /** Asks about removing a module: the dialog explains what blocks it (games in the library
   *  that still need it) or asks to confirm. RetroArch takes every core with it. */
  requestRemove(id: string): void {
    const modules = this.status?.modules ?? [];
    const module = modules.find((m) => m.id === id);
    if (!module) {
      return;
    }
    const affected = id === "retroarch" ? modules.map((m) => m.id) : [id];
    const games = (libraryStore.view?.games ?? [])
      .filter((g) => g.modules.some((m) => affected.includes(m)))
      .sort((a, b) => a.game.name.localeCompare(b.game.name));
    this.removal = {
      id,
      title: module.title,
      alsoRemoved: id === "retroarch" ? modules.filter((m) => m.id !== id && m.installed === true).map((m) => m.short) : [],
      games,
      unmanaged: module.installed === true && module.removable === false,
      records: [...new Set(modules.filter((m) => affected.includes(m.id)).flatMap((m) => m.records))],
    };
  }

  /** Shows the games blocking the pending removal in the Library, filtered by the module. */
  showBlockingGames(): void {
    const removal = this.removal;
    this.removal = null;
    if (removal) {
      libraryStore.showEmulator(removal.id);
      navigation.go("library");
    }
  }

  /** Starts the confirmed removal; its progress window opens over the current page. */
  async confirmRemove(): Promise<boolean> {
    const removal = this.removal;
    this.removal = null;
    if (!removal || removal.games.length > 0 || removal.unmanaged) {
      return false;
    }
    if (!inTauri) {
      toasts.info("Removing modules works inside the desktop app only.");
      return false;
    }
    return flowStore.begin({ kind: "remove_modules", ids: [removal.id] });
  }

  /** Downloads what is missing from the cache, then starts the install flow for `ids`;
   *  its progress window opens over the current page. */
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
    return flowStore.begin({ kind: "install_modules", ids });
  }
}

export const modulesStore = new ModulesStore();
