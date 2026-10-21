/** User preferences, persisted in localStorage (the webview's storage lives in the app's
 *  data directory, so this survives restarts without a plugin). */

export type Theme = "system" | "light" | "dark";

const STORAGE_KEY = "cartload.preferences";

interface Stored {
  theme?: Theme;
  showDebugLog?: boolean;
  logOpen?: boolean;
}

function isTheme(value: unknown): value is Theme {
  return value === "system" || value === "light" || value === "dark";
}

/** Applies a theme: the system preference when no attribute is set, else an override. */
export function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") {
    return;
  }
  const root = document.documentElement;
  if (theme === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", theme);
  }
}

class Preferences {
  theme = $state<Theme>("system");
  showDebugLog = $state(false);
  logOpen = $state(false);

  /** Reads the stored values and applies the theme. Safe to call more than once. */
  load(): void {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const stored = JSON.parse(raw) as Stored;
        if (isTheme(stored.theme)) {
          this.theme = stored.theme;
        }
        if (typeof stored.showDebugLog === "boolean") {
          this.showDebugLog = stored.showDebugLog;
        }
        if (typeof stored.logOpen === "boolean") {
          this.logOpen = stored.logOpen;
        }
      }
    } catch {
      // Unreadable storage: keep the defaults.
    }
    // `?theme=light|dark` forces a theme for previews and screenshots, without storing it.
    const forced = typeof location === "undefined" ? null : new URLSearchParams(location.search).get("theme");
    if (isTheme(forced) && forced !== "system") {
      this.theme = forced;
    }
    applyTheme(this.theme);
  }

  setTheme(theme: Theme): void {
    this.theme = theme;
    applyTheme(theme);
    this.save();
  }

  setShowDebugLog(value: boolean): void {
    this.showDebugLog = value;
    this.save();
  }

  setLogOpen(value: boolean): void {
    this.logOpen = value;
    this.save();
  }

  private save(): void {
    const stored: Stored = {
      theme: this.theme,
      showDebugLog: this.showDebugLog,
      logOpen: this.logOpen,
    };
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
    } catch {
      // Storage full or unavailable: the preference still applies for this run.
    }
  }
}

export const preferences = new Preferences();
