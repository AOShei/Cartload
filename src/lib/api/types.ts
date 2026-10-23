/** Mirrors of the Rust payloads in `src-tauri/src/` (status.rs, logging.rs, commands.rs). */

export type Tone = "neutral" | "ready" | "busy" | "warning" | "error";

export interface Detail {
  label: string;
  value: string;
}

export interface UsbDevice {
  bus: string;
  address: number;
  vendor_id: number;
  product_id: number;
  manufacturer: string | null;
  product: string | null;
  serial: string | null;
}

export type FelCheck =
  | { kind: "unchecked" }
  | { kind: "fel"; soc_id: number; soc: string; r16: boolean }
  | { kind: "not_fel"; reason: string }
  | { kind: "unavailable"; reason: string };

export type Hint =
  | { kind: "starting" }
  | { kind: "nothing" }
  | { kind: "link_no_answer"; interface: string }
  | { kind: "connect_failed"; address: string; reason: string; refused: boolean }
  | { kind: "probe_failed"; reason: string };

export type Link =
  | { kind: "usb"; interface: string; host_address: string }
  | { kind: "network" };

export type BootPhase = "network" | "connecting" | "identifying" | "leaving_fel";

export interface Versions {
  boot: string;
  kernel: string;
  script: string;
}

export interface StorageUsage {
  total: number;
  used: number;
  free: number;
}

export interface GamesUsage {
  all: number;
  non_multiboot: number;
  collections: Record<string, number>;
  extra: number;
}

export interface StorageReport {
  path: string;
  usage: StorageUsage;
  saves: number;
  external_saves: boolean;
  games: GamesUsage;
}

export type Firmware = "Nand" | { Custom: string };

/** `cartload_console::ConsoleInfo`. */
export interface ConsoleInfo {
  hwid: string;
  md_partitioning: boolean;
  minimal_memboot: boolean;
  versions: Versions | null;
  can_interact: boolean;
  console: string;
  system_code: string | null;
  firmware: Firmware | null;
  game_sync_path: string | null;
  original_games_path: string | null;
  rootfs_path: string | null;
  squashfs_path: string | null;
  config: Record<string, string>;
  storage: StorageReport | null;
  original_games: string[];
}

export type ConsoleStatus =
  | { state: "disconnected"; hint: Hint }
  | { state: "fel"; device: UsbDevice; check: FelCheck }
  | { state: "booting"; phase: BootPhase; address: string | null; since: number }
  | { state: "connected"; address: string; link: Link; since: number; info: ConsoleInfo }
  | {
      state: "working";
      flow: string;
      title: string;
      step: string | null;
      index: number | null;
      total: number;
    };

/** Payload of the `console-status` event and of `console_status` / `console_info`. */
export interface StatusReport {
  status: ConsoleStatus;
  tone: Tone;
  title: string;
  next: string;
  details: Detail[];
  /** False for the Mega Drive Mini, which Cartload does not support. */
  supported: boolean;
  changed_at: number;
}

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

/** Payload of the `log` event and of `log_recent`. */
export interface LogLine {
  seq: number;
  time: number;
  level: LogLevel;
  source: string;
  message: string;
}

export interface AppVersion {
  app: string;
  crates: string[];
  features: string[];
  debug: boolean;
}

/** The Linux system setup: udev rule and NetworkManager profile. */
export interface SetupStatus {
  platform: string;
  applies: boolean;
  usb_rule: boolean;
  name_rule: boolean;
  network_manager: boolean;
  network_profile: boolean;
  firewalld: boolean;
  firewall_zone: string | null;
  can_install: boolean;
  complete: boolean;
}

export interface LogFileInfo {
  path: string | null;
  exists: boolean;
  size: number;
}

export interface FelProbe {
  device: UsbDevice;
  check: FelCheck;
}

export interface ExecResult {
  exit_code: number | null;
  stdout: string;
  stderr: string;
}

/** `FlowKind` in flow.rs: which flow to run and its inputs. */
export type FlowKind =
  | { kind: "install" }
  | { kind: "memboot" }
  | { kind: "uninstall"; stock_kernel?: string | null; force_stock_kernel?: boolean }
  | { kind: "dump_kernel"; path: string }
  | { kind: "install_modules"; ids: string[] };

export type StepState = "pending" | "running" | "done" | "failed" | "skipped";

export interface Transfer {
  label: string;
  done: number;
  total: number;
}

/** One step of a flow as the Kernel page shows it. */
export interface StepSnapshot {
  index: number;
  name: string;
  label: string;
  description: string;
  state: StepState;
  started_at: number | null;
  elapsed_ms: number | null;
  message: string | null;
  transfer: Transfer | null;
}

export type FlowState = "idle" | "running" | "done" | "failed" | "cancelled";

export type FailureKind =
  | "timeout"
  | "wrong_state"
  | "precondition"
  | "device"
  | "usb"
  | "connection"
  | "package"
  | "file"
  | "internal";

/** A failed flow, explained (failure.rs). */
export interface FlowFailure {
  step_index: number | null;
  step_name: string | null;
  kind: FailureKind;
  title: string;
  cause: string;
  next: string;
  detail: string;
  wants_stock_kernel: boolean;
  wants_force: boolean;
}

/** Payload of the `flow-progress` event and of `flow_status` / `flow_start`. */
export interface FlowSnapshot {
  seq: number;
  kind: FlowKind | null;
  state: FlowState;
  steps: StepSnapshot[];
  current: number | null;
  message: string | null;
  failure: FlowFailure | null;
  cancel_pending: boolean;
  started_at: number | null;
  finished_at: number | null;
}

export type PackageSource = "override" | "cached";

export interface PackageInfo {
  path: string;
  source: PackageSource;
  version: string;
  boot: string | null;
  kernel: string | null;
  len: number;
  modified: number | null;
}

/** What the Kernel page shows in step 2 (package.rs). */
export interface PackageStatus {
  package: PackageInfo | null;
  cache_path: string | null;
  sources: string[];
  problem: string | null;
}

/** Payload of the `package-download` event. */
export interface DownloadProgress {
  done: number;
  total: number | null;
}

/* Library page (library.rs, sync.rs) */

export type System = "snes" | "nes" | "mega-drive";

/** `cartload_library::Game` (game.json). */
export interface Game {
  code: string;
  name: string;
  sort_name: string;
  system: System;
  rom_file: string;
  crc32: number;
  file_crc32: number;
  original_file: string;
  players: number;
  simultaneous: boolean;
  release_date: string;
  publisher: string;
  copyright: string;
  save_count: number;
  application: string;
  args: string;
  tgdb_ids: number[];
  tgdb_id: number | null;
  description: string;
  db_name: string | null;
  snes: unknown;
  nes: unknown;
  placeholder_art: boolean;
  /** The ROM goes to the console as a 7z archive (needs the tiny7zx mod there). */
  compress: boolean;
}

export interface GameView {
  game: Game;
  size: number;
  cover: string;
  thumbnail: string;
  system_name: string;
}

export interface LibraryView {
  root: string;
  games: GameView[];
  total_size: number;
}

export type Position = "left-back" | "leftmost" | "left" | "right" | "rightmost" | "back";

export type FolderImage = "folder" | "back" | "home" | "number" | { letter: string } | { custom: string };

export interface MenuFolder {
  kind: "folder";
  name: string;
  position: Position;
  image: FolderImage;
  entries: MenuEntry[];
}

export type MenuEntry =
  | { kind: "game"; code: string }
  | { kind: "original"; code: string }
  | MenuFolder
  | { kind: "back"; name: string; position: Position; image: FolderImage; target: "parent" | "root" };

export type Structure = { kind: "flat" } | { kind: "letters" } | { kind: "count"; max: number };

export interface MenuOptions {
  structure: Structure;
  originals_at_root: boolean;
}

export interface OriginalView {
  code: string;
  name: string;
  /** URL of the firmware's cover, when cached from a console. */
  cover: string | null;
  /** URL of the firmware's thumbnail, when cached from a console. */
  thumbnail: string | null;
}

export interface MenuView {
  entries: MenuEntry[];
  options: MenuOptions;
  custom: boolean;
  system_code: string | null;
  originals: OriginalView[];
  /** The pictures chosen for folders, by stem. */
  folder_art: Record<string, { cover: string; thumbnail: string }>;
}

/** A path of entry indices from the root menu; `[]` is the root itself. */
export type MenuPath = number[];

export type MenuItem = { kind: "game"; code: string } | { kind: "folder"; path: MenuPath };

export type MenuEdit =
  | { op: "add_folder"; parent: MenuPath; name: string }
  | { op: "rename_folder"; path: MenuPath; name: string }
  | { op: "remove_folder"; path: MenuPath }
  | { op: "set_folder_image"; path: MenuPath; file: string | null };

export interface ImportedGame {
  code: string;
  name: string;
  system: string;
  matched: boolean;
  replaced: boolean;
  warnings: string[];
  notes: string[];
}

export type ImportOutcome =
  | { state: "imported"; games: ImportedGame[] }
  | { state: "failed"; error: string }
  | { state: "skipped"; reason: string };

/** Payload of the `library-import` event: before (no outcome) and after each file. */
export interface ImportEvent {
  index: number;
  total: number;
  path: string;
  outcome: ImportOutcome | null;
}

export interface ImportReport {
  files: ImportEvent[];
  imported: number;
  unmatched: number;
  failed: number;
  skipped: number;
  codes: string[];
}

export interface GameEdit {
  code: string;
  name: string;
  sort_name: string;
  publisher: string;
  release_date: string;
  players: number;
  simultaneous: boolean;
  copyright: string;
  description: string;
  application: string;
  args: string;
  /** Compress the ROM for the console; omitted leaves the setting alone. */
  compress?: boolean;
}

export type MatchSource = "hash_db" | "search" | "manual";

export type ArtEvent =
  | { event: "searching"; code: string; query: string }
  | { event: "matched"; code: string; id: number; title: string; source: MatchSource }
  | { event: "no_match"; code: string; queries: string[] }
  | { event: "fetching"; code: string | null; what: string }
  | { event: "done"; code: string; outcome: ArtGameOutcome }
  | { event: "failed"; code: string; error: string };

export type CoverStatus = "unchanged" | "written" | "no_box_art" | "no_match";

export interface ArtGameOutcome {
  id: number | null;
  title: string | null;
  source: MatchSource | null;
  cover: CoverStatus;
  cover_size: [number, number] | null;
  thumbnail_size: [number, number] | null;
  from_cache: boolean;
  logo: string;
  metadata: string[];
}

/** Payload of the `art-progress` event. */
export interface ArtProgress {
  done: number;
  total: number;
  event: ArtEvent;
}

export interface ArtGameResult {
  code: string;
  outcome: ArtGameOutcome | null;
  error: string | null;
}

export interface Allowance {
  remaining: number | null;
  extra: number;
  refresh_at: number | null;
  updated_at: number;
}

export interface ArtOutcome {
  report: { results: ArtGameResult[]; allowance: Allowance };
  summary: string;
}

export interface KeyStatus {
  stored: boolean;
  environment: boolean;
  remaining: number | null;
}

export interface FileView {
  path: string;
  size: number;
}

export interface StorageView {
  total: number;
  used: number;
  free: number;
}

export interface SyncPlanView {
  system_code: string;
  upload_path: string;
  games: number;
  originals: number;
  folders: number;
  files: number;
  uploads: FileView[];
  deletions: string[];
  link_deletions: string[];
  empty_dirs: string[];
  unchanged: number;
  total_size: number;
  upload_size: number;
  available: number | null;
  fits: boolean;
  noop: boolean;
  changes: number;
  storage: StorageView | null;
  planned_at: number;
  nes_via_retroarch: number;
  warnings: string[];
}

export type SyncState = "idle" | "running" | "done" | "failed";

export interface SyncStep {
  index: number;
  total: number;
  name: string;
  label: string;
  detail: string;
}

export interface UploadView {
  path: string;
  done: number;
  total: number;
  overall_done: number;
  overall_total: number;
}

export interface SyncReportView {
  uploaded_files: number;
  uploaded_bytes: number;
  removed_files: number;
  removed_links: number;
  removed_dirs: number;
  games_mounted: boolean;
}

/** Payload of the `sync-progress` event and of `sync_run` / `sync_status`. */
export interface SyncSnapshot {
  seq: number;
  state: SyncState;
  plan: SyncPlanView | null;
  step: SyncStep | null;
  upload: UploadView | null;
  log: string[];
  report: SyncReportView | null;
  error: { message: string; detail: string | null } | null;
  started_at: number | null;
  finished_at: number | null;
}

/* Emulator modules (RetroArch and cores) from the Mod Hub */

export interface ModuleView {
  id: string;
  item: string;
  title: string;
  purpose: string;
  approx_bytes: number;
  cached: boolean;
  cached_bytes: number | null;
  installed: boolean | null;
}

export interface ModulesStatus {
  cache_dir: string | null;
  hub: string;
  modules: ModuleView[];
  console_checked: boolean;
}

export interface ModuleDownload {
  id: string;
  done: number;
  total: number | null;
}
