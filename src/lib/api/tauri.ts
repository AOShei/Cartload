import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Whether the page runs inside the Tauri shell (as opposed to `pnpm dev` in a browser). */
export const inTauri: boolean =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** A rejected command: one sentence plus the raw detail (`CommandError` in error.rs). */
export class AppError extends Error {
  readonly detail: string | null;

  constructor(message: string, detail: string | null = null) {
    super(message);
    this.name = "AppError";
    this.detail = detail;
  }

  /** Normalises whatever `invoke` rejected with. */
  static from(value: unknown): AppError {
    if (value instanceof AppError) {
      return value;
    }
    if (typeof value === "object" && value !== null && "message" in value) {
      const message = (value as { message: unknown }).message;
      const detail = (value as { detail?: unknown }).detail;
      if (typeof message === "string") {
        return new AppError(message, typeof detail === "string" ? detail : null);
      }
    }
    if (value instanceof Error) {
      return new AppError(value.message, value.stack ?? null);
    }
    return new AppError(String(value));
  }
}

/** Invokes a backend command; rejects with an {@link AppError}. */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) {
    throw new AppError(
      "Not running inside the desktop app.",
      `The command ${command} needs the Tauri shell.`,
    );
  }
  try {
    return await invoke<T>(command, args);
  } catch (error: unknown) {
    throw AppError.from(error);
  }
}

/** Subscribes to a backend event. Outside Tauri this is a no-op. */
export async function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return () => {};
  }
  return listen<T>(event, (e) => handler(e.payload));
}
