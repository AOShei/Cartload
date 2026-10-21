import { invoke } from "@tauri-apps/api/core";

/** Mirror of `AppVersion` in `src-tauri/src/lib.rs`. */
export interface AppVersion {
  app: string;
  crates: string[];
}

/** Ask the Rust side for its version. Rejects when not running inside Tauri. */
export function fetchAppVersion(): Promise<AppVersion> {
  return invoke<AppVersion>("app_version");
}
