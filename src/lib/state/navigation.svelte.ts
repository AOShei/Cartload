import type { IconName } from "../components/icons";

export type PageId = "console" | "kernel" | "library" | "settings";

export interface PageDef {
  id: PageId;
  label: string;
  icon: IconName;
  /** One line shown under the page title. */
  hint: string;
}

/** The pages, in sidebar order. */
export const PAGES: readonly PageDef[] = [
  { id: "console", label: "Console", icon: "console", hint: "What is connected and what it can do." },
  { id: "kernel", label: "Kernel", icon: "kernel", hint: "Install or repair hakchi on the console." },
  { id: "library", label: "Library", icon: "library", hint: "Your games, folders and art." },
  { id: "settings", label: "Settings", icon: "settings", hint: "Theme and advanced options." },
];

class Navigation {
  page = $state<PageId>("console");

  go(page: PageId): void {
    this.page = page;
  }

  /** The definition of the current page. */
  get current(): PageDef {
    return PAGES.find((p) => p.id === this.page) ?? PAGES[0]!;
  }
}

export const navigation = new Navigation();
