/** Inline SVG icons (docs/design.md: no external assets). Paths on a 24 px grid, stroked. */
export const ICONS = {
  console: "M7 3h10a1 1 0 0 1 1 1v17H6V4a1 1 0 0 1 1-1zM9 3v4h6V3M9 11h6M9 15h6",
  kernel:
    "M8 8h8v8H8zM4 10h4M4 14h4M16 10h4M16 14h4M10 4v4M14 4v4M10 16v4M14 16v4",
  library: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  settings: "M4 7h9M17 7h3M4 17h3M11 17h9M15 7m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0M9 17m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0",
  refresh: "M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7",
  close: "M6 6l12 12M18 6L6 18",
  chevron: "M9 6l6 6-6 6",
  warning: "M12 3l10 18H2zM12 10v5M12 18h.01",
  info: "M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0-18 0M12 11v5M12 8h.01",
  check: "M5 12l5 5L20 7",
  log: "M4 6h16M4 12h10M4 18h14",
  usb: "M12 20V4M9 7l3-3 3 3M12 14l-4-2v-2M12 16l4-2v-3M12 20m-1.5 0a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0",
  play: "M7 5l12 7-12 7z",
  copy: "M9 9h11v11H9zM5 15V5a1 1 0 0 1 1-1h10",
  trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  sun: "M12 12m-4 0a4 4 0 1 0 8 0a4 4 0 1 0-8 0M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
  moon: "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
  arrow: "M5 12h14M13 6l6 6-6 6",
  stop: "M7 7h10v10H7z",
  download: "M12 4v11M7 10l5 5 5-5M5 19h14",
  more: "M6 12h.01M12 12h.01M18 12h.01",
  save: "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
  circle: "M12 12m-7 0a7 7 0 1 0 14 0a7 7 0 1 0-14 0",
  dash: "M7 12h10",
  undo: "M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
} as const;

export type IconName = keyof typeof ICONS;
