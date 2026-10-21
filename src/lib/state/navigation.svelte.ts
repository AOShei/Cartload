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

function isPageId(value: string | null): value is PageId {
  return PAGES.some((p) => p.id === value);
}

/** `?page=kernel` opens a page directly (browser previews and screenshots). */
function initialPage(): PageId {
  if (typeof location === "undefined") {
    return "console";
  }
  const wanted = new URLSearchParams(location.search).get("page");
  return isPageId(wanted) ? wanted : "console";
}

class Navigation {
  page = $state<PageId>(initialPage());
  /** A section of the page to scroll to and focus once it is shown, e.g. "about". */
  section = $state<string | null>(null);

  go(page: PageId, section: string | null = null): void {
    this.page = page;
    this.section = section;
  }

  /** The page took the requested section; forget it. */
  tookSection(): void {
    this.section = null;
  }

  /** The definition of the current page. */
  get current(): PageDef {
    return PAGES.find((p) => p.id === this.page) ?? PAGES[0]!;
  }
}

export const navigation = new Navigation();
