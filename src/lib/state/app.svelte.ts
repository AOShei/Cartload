import { fetchAppVersion } from "../api/commands";
import { AppError, inTauri } from "../api/tauri";
import type { AppVersion } from "../api/types";

class AppStore {
  version = $state<AppVersion | null>(null);
  versionError = $state<string | null>(null);

  /** Whether the backend was built with the `dev-exec` feature. */
  get devExec(): boolean {
    return this.version?.features.includes("dev-exec") ?? false;
  }

  async load(): Promise<void> {
    if (!inTauri) {
      this.versionError = "browser preview";
      return;
    }
    try {
      this.version = await fetchAppVersion();
    } catch (error: unknown) {
      this.versionError = AppError.from(error).message;
    }
  }
}

export const app = new AppStore();
