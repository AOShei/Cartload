import type { StatusReport } from "../../api/types";

export type Tone = "neutral" | "ready" | "busy" | "warning" | "error";

/** What the setup needs to know about the console, as one line. */
export interface Watch {
  tone: Tone;
  text: string;
  /** A flow that boots hakchi can start: FEL mode, the memboot shell or a running hakchi. */
  ready: boolean;
  /** The console runs its installed hakchi. */
  installed: boolean;
}

/** The console as the setup and maintenance actions see it. */
export function consoleWatch(report: StatusReport | null): Watch {
  const waiting: Watch = { tone: "neutral", text: "Waiting for the console on USB…", ready: false, installed: false };
  const status = report?.status;
  if (!status) {
    return waiting;
  }
  switch (status.state) {
    case "fel":
      switch (status.check.kind) {
        case "fel":
          return status.check.r16
            ? { tone: "ready", text: "Console in FEL mode. Ready.", ready: true, installed: false }
            : {
                tone: "warning",
                text: `A FEL device is attached, but its chip is ${status.check.soc}, not a Classic Mini's.`,
                ready: false,
                installed: false,
              };
        case "not_fel":
          return {
            tone: "warning",
            text: "Console booted normally. Switch it off and follow the steps.",
            ready: false,
            installed: false,
          };
        case "unavailable":
          return { tone: "error", text: report?.title ?? "", ready: false, installed: false };
        default:
          return { tone: "busy", text: "Checking the attached console…", ready: false, installed: false };
      }
    case "connected":
      if (report && !report.supported) {
        return { tone: "warning", text: report.title, ready: false, installed: false };
      }
      if (status.info.minimal_memboot) {
        return { tone: "ready", text: "Console running the custom system from memory. Ready.", ready: true, installed: false };
      }
      return {
        tone: "ready",
        text: "Console connected and running the custom system. Ready, no Reset needed.",
        ready: true,
        installed: true,
      };
    case "booting":
      return { tone: "busy", text: report?.title ?? "Console coming up…", ready: false, installed: false };
    case "working":
      return { tone: "busy", text: "In use by the running flow.", ready: true, installed: false };
    default:
      return waiting;
  }
}

/** A console running its installed hakchi that Cartload can talk to: emulators and syncs
 *  work there. */
export function canManage(report: StatusReport | null): boolean {
  const status = report?.status;
  return (
    status?.state === "connected" &&
    status.info.can_interact &&
    !status.info.minimal_memboot &&
    (report?.supported ?? true)
  );
}
