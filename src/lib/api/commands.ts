import { call } from "./tauri";
import type {
  AppVersion,
  ExecResult,
  FelProbe,
  FlowKind,
  FlowSnapshot,
  LogLine,
  PackageStatus,
  StatusReport,
  StepSnapshot,
} from "./types";

/** Version of the Rust side and the crates linked into it. */
export function fetchAppVersion(): Promise<AppVersion> {
  return call<AppVersion>("app_version");
}

/** The latest status report. */
export function consoleStatus(): Promise<StatusReport> {
  return call<StatusReport>("console_status");
}

/** Looks at the console now and re-reads its details when connected. */
export function consoleInfo(): Promise<StatusReport> {
  return call<StatusReport>("console_info");
}

/** Opens the attached FEL device and reads its version block. */
export function felProbe(): Promise<FelProbe> {
  return call<FelProbe>("fel_probe");
}

/** The buffered log lines, oldest first. */
export function logRecent(): Promise<LogLine[]> {
  return call<LogLine[]>("log_recent");
}

/** Runs a shell line on the console. Only in builds with the `dev-exec` feature. */
export function consoleExec(command: string): Promise<ExecResult> {
  return call<ExecResult>("console_exec", { command });
}

/** Starts a flow; progress follows as `flow-progress` events. */
export function flowStart(kind: FlowKind): Promise<FlowSnapshot> {
  return call<FlowSnapshot>("flow_start", { kind });
}

/** Asks the running flow to stop (after the current step when it writes to flash). */
export function flowCancel(): Promise<FlowSnapshot> {
  return call<FlowSnapshot>("flow_cancel");
}

/** The latest flow snapshot. */
export function flowStatus(): Promise<FlowSnapshot> {
  return call<FlowSnapshot>("flow_status");
}

/** The steps a flow would run, all pending. */
export function flowSteps(kind: FlowKind): Promise<StepSnapshot[]> {
  return call<StepSnapshot[]>("flow_steps", { kind });
}

/** The kernel package that would be installed. */
export function packageInfo(): Promise<PackageStatus> {
  return call<PackageStatus>("package_info");
}

/** Downloads the latest kernel package; progress follows as `package-download` events. */
export function packageUpdate(): Promise<PackageStatus> {
  return call<PackageStatus>("package_update");
}

/* Library page */

import type {
  ArtOutcome,
  GameEdit,
  GameView,
  ImportReport,
  KeyStatus,
  LibraryView,
  MenuEdit,
  MenuItem,
  MenuOptions,
  MenuPath,
  MenuView,
  SyncPlanView,
  SyncSnapshot,
} from "./types";

/** Every game with its size and cover URL. */
export function libraryList(): Promise<LibraryView> {
  return call<LibraryView>("library_list");
}

/** Imports files and folders; progress follows as `library-import` events. */
export function libraryImport(paths: string[], folder: MenuPath | null): Promise<ImportReport> {
  return call<ImportReport>("library_import", { paths, folder });
}

/** Saves the editable fields of a game. */
export function libraryUpdate(edit: GameEdit): Promise<GameView> {
  return call<GameView>("library_update", { edit });
}

/** Removes games from the library. */
export function libraryRemove(codes: string[]): Promise<number> {
  return call<number>("library_remove", { codes });
}

/** The folder tree the console will show. */
export function menuGet(): Promise<MenuView> {
  return call<MenuView>("menu_get");
}

/** Applies an automatic structure. */
export function menuSetStructure(options: MenuOptions): Promise<MenuView> {
  return call<MenuView>("menu_set_structure", { options });
}

/** Moves games and folders into a folder. */
export function menuMove(items: MenuItem[], target: MenuPath, index: number | null): Promise<MenuView> {
  return call<MenuView>("menu_move", { items, target, index });
}

/** Adds, renames or removes a folder. */
export function menuEdit(edit: MenuEdit): Promise<MenuView> {
  return call<MenuView>("menu_edit", { edit });
}

/** Fetches art and metadata; progress follows as `art-progress` events. */
export function artFetch(codes: string[], force: boolean): Promise<ArtOutcome> {
  return call<ArtOutcome>("art_fetch", { codes, force });
}

/** Uses an image file as a game's cover. */
export function artSetCover(code: string, path: string): Promise<GameView> {
  return call<GameView>("art_set_cover", { code, path });
}

/** Where the TheGamesDB key comes from. */
export function tgdbKeyGet(): Promise<KeyStatus> {
  return call<KeyStatus>("tgdb_key_get");
}

/** Stores or clears the TheGamesDB key. */
export function tgdbKeySet(key: string | null): Promise<KeyStatus> {
  return call<KeyStatus>("tgdb_key_set", { key });
}

/** What a sync would do, against the connected console. */
export function syncPlan(): Promise<SyncPlanView> {
  return call<SyncPlanView>("sync_plan");
}

/** Runs a sync; progress follows as `sync-progress` events. */
export function syncRun(): Promise<SyncSnapshot> {
  return call<SyncSnapshot>("sync_run");
}

/** The latest sync snapshot. */
export function syncStatus(): Promise<SyncSnapshot> {
  return call<SyncSnapshot>("sync_status");
}
