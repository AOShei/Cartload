/** Non-modal notifications. Errors stay longer and carry their raw detail. */

export type ToastKind = "error" | "info" | "success";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  detail: string | null;
}

const ERROR_TTL = 12_000;
const INFO_TTL = 5_000;

class Toasts {
  items = $state<Toast[]>([]);
  private nextId = 1;
  private timers = new Map<number, ReturnType<typeof setTimeout>>();

  error(message: string, detail: string | null = null): void {
    this.push("error", message, detail, ERROR_TTL);
  }

  info(message: string, detail: string | null = null): void {
    this.push("info", message, detail, INFO_TTL);
  }

  success(message: string): void {
    this.push("success", message, null, INFO_TTL);
  }

  dismiss(id: number): void {
    const timer = this.timers.get(id);
    if (timer) {
      clearTimeout(timer);
      this.timers.delete(id);
    }
    this.items = this.items.filter((toast) => toast.id !== id);
  }

  private push(kind: ToastKind, message: string, detail: string | null, ttl: number): void {
    // The same message twice in a row only resets the timer.
    const existing = this.items.find((t) => t.kind === kind && t.message === message);
    if (existing) {
      this.dismiss(existing.id);
    }
    const id = this.nextId++;
    this.items.push({ id, kind, message, detail });
    this.timers.set(
      id,
      setTimeout(() => this.dismiss(id), ttl),
    );
  }
}

export const toasts = new Toasts();
