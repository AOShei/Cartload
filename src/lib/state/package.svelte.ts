import { packageInfo, packageUpdate } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { DownloadProgress, PackageStatus } from "../api/types";
import { previewPackage } from "../dev/preview";
import { preferences } from "./preferences.svelte";
import { toasts } from "./toasts.svelte";

/** `9.3 MB`, `761 KB`, `14 B` (binary units, decimal labels, as in status.rs). */
export function formatBytes(bytes: number): string {
  const kib = 1024;
  if (bytes >= kib * kib * kib) {
    return `${(bytes / (kib * kib * kib)).toFixed(1)} GB`;
  }
  if (bytes >= kib * kib) {
    return `${(bytes / (kib * kib)).toFixed(1)} MB`;
  }
  if (bytes >= kib) {
    return `${Math.round(bytes / kib)} KB`;
  }
  return `${bytes} B`;
}

class PackageStore {
  status = $state<PackageStatus | null>(null);
  loading = $state(false);
  updating = $state(false);
  progress = $state<DownloadProgress | null>(null);
  /** Why the last download failed, shown in place until the next attempt. */
  error = $state<string | null>(null);
  private started = false;

  /** Subscribes to download progress and reads the current status. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    if (!inTauri) {
      this.status = previewPackage();
      return;
    }
    await on<DownloadProgress>("package-download", (progress) => {
      this.progress = progress;
    });
    await this.load();
  }

  async load(): Promise<void> {
    if (!inTauri || this.loading) {
      return;
    }
    this.loading = true;
    try {
      this.status = await packageInfo(preferences.kernelPackage);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the kernel package: ${e.message}`, e.detail);
    } finally {
      this.loading = false;
    }
  }

  /** Chooses the package to install (by id) and reads its status. */
  async choose(id: string): Promise<void> {
    preferences.setKernelPackage(id);
    if (inTauri) {
      await this.load();
    }
  }

  /** What the user sees for a package: `Cartload system 0.1.0` or `hakchi v1.0.4-126`. */
  label(): string {
    const pkg = this.status?.package;
    if (!pkg) {
      return "";
    }
    return pkg.system ? `Cartload system ${pkg.system}` : `hakchi ${pkg.version}`;
  }

  /** Downloads the pinned package into the cache (fallback for the built-in copy). */
  async update(): Promise<void> {
    if (this.updating) {
      return;
    }
    if (!inTauri) {
      toasts.info("Downloads run inside the desktop app only.");
      return;
    }
    this.updating = true;
    this.progress = { done: 0, total: null };
    this.error = null;
    try {
      this.status = await packageUpdate(preferences.kernelPackage);
      toasts.success(`Kernel package ${this.label() || "unknown"} is ready.`);
    } catch (error: unknown) {
      const e = AppError.from(error);
      this.error = e.message;
      toasts.error(e.message, e.detail);
    } finally {
      this.updating = false;
      this.progress = null;
    }
  }
}

export const packageStore = new PackageStore();
