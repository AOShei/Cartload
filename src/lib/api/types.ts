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
  | { kind: "dump_kernel"; path: string };

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
