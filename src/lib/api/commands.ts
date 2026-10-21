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
