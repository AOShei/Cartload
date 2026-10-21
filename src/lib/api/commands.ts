import { call } from "./tauri";
import type { AppVersion, ExecResult, FelProbe, LogLine, StatusReport } from "./types";

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
