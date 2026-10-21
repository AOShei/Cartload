import { packageInfo, packageUpdate } from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type { DownloadProgress, PackageStatus } from "../api/types";
import { previewPackage } from "../dev/preview";
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
      this.status = await packageInfo();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the kernel package: ${e.message}`, e.detail);
    } finally {
      this.loading = false;
    }
  }

  /** Downloads the latest package into the cache. */
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
    try {
      this.status = await packageUpdate();
      const version = this.status.package?.version ?? "unknown";
      toasts.success(`Kernel package ${version} is ready.`);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.updating = false;
      this.progress = null;
    }
  }
}

export const packageStore = new PackageStore();
