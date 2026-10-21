/** The top-level areas of the app, shown in the sidebar. */
export const SECTIONS = ["Console", "Library", "Kernel"] as const;

export type Section = (typeof SECTIONS)[number];
