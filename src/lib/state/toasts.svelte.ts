/** Non-modal notifications. Errors stay longer and carry their raw detail; a toast can offer
 *  one action, which is how destructive actions get their inline undo. */

export type ToastKind = "error" | "info" | "success";

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  detail: string | null;
  action: ToastAction | null;
}

const ERROR_TTL = 12_000;
const INFO_TTL = 5_000;
const UNDO_TTL = 8_000;

class Toasts {
  items = $state<Toast[]>([]);
  private nextId = 1;
  private timers = new Map<number, ReturnType<typeof setTimeout>>();

  error(message: string, detail: string | null = null): void {
    this.push("error", message, detail, null, ERROR_TTL);
  }

  info(message: string, detail: string | null = null): void {
    this.push("info", message, detail, null, INFO_TTL);
  }

  success(message: string): void {
    this.push("success", message, null, null, INFO_TTL);
  }

  /** A success message with an Undo button that runs `undo` and dismisses the toast. */
  undoable(message: string, undo: () => void): void {
    this.push("success", message, null, { label: "Undo", run: undo }, UNDO_TTL);
  }

  dismiss(id: number): void {
    const timer = this.timers.get(id);
    if (timer) {
      clearTimeout(timer);
      this.timers.delete(id);
    }
    this.items = this.items.filter((toast) => toast.id !== id);
  }

  /** Runs a toast's action and puts the toast away. */
  act(id: number): void {
    const toast = this.items.find((t) => t.id === id);
    this.dismiss(id);
    toast?.action?.run();
  }

  private push(
    kind: ToastKind,
    message: string,
    detail: string | null,
    action: ToastAction | null,
    ttl: number,
  ): void {
    // The same message twice in a row only resets the timer.
    const existing = this.items.find((t) => t.kind === kind && t.message === message);
    if (existing) {
      this.dismiss(existing.id);
    }
    const id = this.nextId++;
    this.items.push({ id, kind, message, detail, action });
    this.timers.set(
      id,
      setTimeout(() => this.dismiss(id), ttl),
    );
  }
}

export const toasts = new Toasts();
