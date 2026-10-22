import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  artFetch,
  artSetCover,
  libraryImport,
  libraryList,
  libraryRemove,
  libraryUpdate,
  menuEdit,
  menuGet,
  menuMove,
  menuSetStructure,
} from "../api/commands";
import { AppError, inTauri, on } from "../api/tauri";
import type {
  ArtProgress,
  GameEdit,
  GameView,
  ImportEvent,
  ImportReport,
  MenuEdit,
  MenuEntry,
  MenuFolder,
  MenuItem,
  MenuOptions,
  MenuPath,
  MenuView,
  LibraryView,
} from "../api/types";
import { previewLibrary } from "../dev/preview";
import { navigation } from "./navigation.svelte";
import { toasts } from "./toasts.svelte";

export type ViewMode = "grid" | "list";

/** One thing shown in the grid: a game, a firmware game or a folder. */
export type Tile =
  | { kind: "game"; id: string; view: GameView }
  | { kind: "original"; id: string; code: string; name: string; cover: string | null; thumbnail: string | null }
  | { kind: "folder"; id: string; path: MenuPath; folder: MenuFolder; games: number; cover: string | null };

export const IMPORT_EXTENSIONS = ["sfc", "smc", "fig", "swc", "nes", "zip"];
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "bmp"];
/** MIME type of an internal drag (games and folders moved between folders). */
export const DRAG_TYPE = "application/x-cartload-items";

const VIEW_KEY = "cartload.library.view";

/** `"0/2"` for a path, `""` for the root menu. */
export function pathKey(path: MenuPath): string {
  return path.join("/");
}

export function samePath(a: MenuPath, b: MenuPath): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

/** The tile id of a folder. */
export function folderId(path: MenuPath): string {
  return `f:${pathKey(path)}`;
}

/** The entries of the menu at `path` (the root for `[]`), or null when the path is gone. */
export function entriesAt(entries: MenuEntry[], path: MenuPath): MenuEntry[] | null {
  let current = entries;
  for (const index of path) {
    const entry = current[index];
    if (!entry || entry.kind !== "folder") {
      return null;
    }
    current = entry.entries;
  }
  return current;
}

/** The folder at `path`, or null for the root and for paths that are gone. */
export function folderAt(entries: MenuEntry[], path: MenuPath): MenuFolder | null {
  if (path.length === 0) {
    return null;
  }
  const parent = entriesAt(entries, path.slice(0, -1));
  const entry = parent?.[path[path.length - 1]!];
  return entry && entry.kind === "folder" ? entry : null;
}

/** Games (library and firmware) in `entries` and below. */
export function countGames(entries: MenuEntry[]): number {
  let count = 0;
  for (const entry of entries) {
    if (entry.kind === "game" || entry.kind === "original") {
      count += 1;
    } else if (entry.kind === "folder") {
      count += countGames(entry.entries);
    }
  }
  return count;
}

/** The names of the folders on the way to `path`, root first. */
export function breadcrumbs(entries: MenuEntry[], path: MenuPath): { name: string; path: MenuPath }[] {
  const crumbs: { name: string; path: MenuPath }[] = [{ name: "Console menu", path: [] }];
  for (let depth = 1; depth <= path.length; depth += 1) {
    const sub = path.slice(0, depth);
    const folder = folderAt(entries, sub);
    crumbs.push({ name: folder?.name ?? "?", path: sub });
  }
  return crumbs;
}

function isLeft(folder: MenuFolder): boolean {
  return folder.position === "left" || folder.position === "leftmost" || folder.position === "left-back";
}

/** The file name without its directory. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

class LibraryStore {
  view = $state<LibraryView | null>(null);
  menu = $state<MenuView | null>(null);
  loading = $state(false);
  /** The first load finished (with or without games). */
  loaded = $state(false);
  error = $state<string | null>(null);
  /** The folder shown in the grid. */
  folder = $state<MenuPath>([]);
  /** Tile ids, in selection order. */
  selected = $state<string[]>([]);
  viewMode = $state<ViewMode>("grid");
  filter = $state("");
  importing = $state(false);
  importFiles = $state<ImportEvent[]>([]);
  importReport = $state<ImportReport | null>(null);
  importOpen = $state(false);
  /** Files from outside the app are being dragged over the window. */
  dragOver = $state(false);
  artBusy = $state(false);
  artProgress = $state<ArtProgress | null>(null);
  saving = $state(false);
  removing = $state(false);
  /** The detail panel shows its remove confirmation (Delete in the grid asks for it). */
  removePrompt = $state(false);
  /** The folder panel shows its removal confirmation. */
  removeFolderPrompt = $state(false);
  /** A folder edit, move or structure change is on its way to the backend. */
  menuBusy = $state(false);
  /** A cover image is being replaced. */
  coverBusy = $state(false);
  /** Bumped on every change to the games or the tree, so the sync plan follows. */
  revision = $state(0);
  private anchor: string | null = null;
  private started = false;

  /** Games by code. */
  get games(): Map<string, GameView> {
    return new Map((this.view?.games ?? []).map((g) => [g.game.code, g]));
  }

  get gameCount(): number {
    return this.view?.games.length ?? 0;
  }

  /** Loaded and nothing in it: the first-run state. */
  get empty(): boolean {
    return this.loaded && this.view !== null && this.view.games.length === 0;
  }

  get currentFolder(): MenuFolder | null {
    return this.menu ? folderAt(this.menu.entries, this.folder) : null;
  }

  get currentEntries(): MenuEntry[] {
    return (this.menu && entriesAt(this.menu.entries, this.folder)) ?? [];
  }

  /** What the console shows in the current folder: left folders, games by title, right
   *  folders (back links are implicit). */
  get tiles(): Tile[] {
    const games = this.games;
    const originals = new Map((this.menu?.originals ?? []).map((o) => [o.code, o]));
    const left: Tile[] = [];
    const middle: Tile[] = [];
    const right: Tile[] = [];
    this.currentEntries.forEach((entry, index) => {
      if (entry.kind === "game") {
        const view = games.get(entry.code);
        if (view) {
          middle.push({ kind: "game", id: entry.code, view });
        }
      } else if (entry.kind === "original") {
        const original = originals.get(entry.code);
        middle.push({
          kind: "original",
          id: entry.code,
          code: entry.code,
          name: original?.name ?? entry.code,
          cover: original?.cover ?? null,
          thumbnail: original?.thumbnail ?? null,
        });
      } else if (entry.kind === "folder") {
        const path = [...this.folder, index];
        const tile: Tile = {
          kind: "folder",
          id: folderId(path),
          path,
          folder: entry,
          games: countGames(entry.entries),
          cover: this.folderCover(entry),
        };
        (isLeft(entry) ? left : right).push(tile);
      }
    });
    middle.sort((a, b) => tileSortKey(a).localeCompare(tileSortKey(b)));
    return [...left, ...middle, ...right];
  }

  /** The tiles matching the filter box. */
  get visibleTiles(): Tile[] {
    const needle = this.filter.trim().toLowerCase();
    if (!needle) {
      return this.tiles;
    }
    return this.tiles.filter((tile) => tileName(tile).toLowerCase().includes(needle));
  }

  get selectedTiles(): Tile[] {
    const byId = new Map(this.tiles.map((t) => [t.id, t]));
    return this.selected.map((id) => byId.get(id)).filter((t): t is Tile => t !== undefined);
  }

  get selectedGames(): GameView[] {
    return this.selectedTiles.flatMap((t) => (t.kind === "game" ? [t.view] : []));
  }

  /** The one selected tile, when exactly one is. */
  get single(): Tile | null {
    const tiles = this.selectedTiles;
    return tiles.length === 1 ? tiles[0]! : null;
  }

  /** Subscribes to import, art and file-drop events and loads the library. */
  async start(): Promise<void> {
    if (this.started) {
      return;
    }
    this.started = true;
    try {
      const stored = localStorage.getItem(VIEW_KEY);
      if (stored === "grid" || stored === "list") {
        this.viewMode = stored;
      }
    } catch {
      // No storage: the default stays.
    }
    if (!inTauri) {
      const fixture = previewLibrary();
      this.view = fixture.view;
      this.menu = fixture.menu;
      this.loaded = true;
      // `?folder=1` opens a folder of the fixture (index path, `/`-separated) for screenshots.
      const wanted = new URLSearchParams(location.search).get("folder");
      if (wanted) {
        const path = wanted.split("/").map(Number);
        if (entriesAt(fixture.menu.entries, path)) {
          this.folder = path;
        }
      }
      // `?select=N` selects the first N games of that folder, to show the detail panel.
      const select = Number.parseInt(new URLSearchParams(location.search).get("select") ?? "", 10);
      if (select > 0) {
        this.selected = this.visibleTiles.filter((t) => t.kind === "game").slice(0, select).map((t) => t.id);
      }
      return;
    }
    await on<ImportEvent>("library-import", (event) => this.onImportEvent(event));
    await on<ArtProgress>("art-progress", (progress) => {
      this.artProgress = progress;
    });
    try {
      await getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          this.dragOver = true;
        } else if (payload.type === "leave") {
          this.dragOver = false;
        } else if (payload.type === "drop") {
          this.dragOver = false;
          navigation.go("library");
          void this.importPaths(payload.paths);
        }
      });
    } catch (error: unknown) {
      toasts.error(`File drops are unavailable: ${AppError.from(error).message}`);
    }
    await this.load();
  }

  /** Reads the games and the tree again. */
  async load(): Promise<void> {
    if (!inTauri || this.loading) {
      return;
    }
    this.loading = true;
    try {
      const [view, menu] = await Promise.all([libraryList(), menuGet()]);
      this.view = view;
      this.applyMenu(menu);
      this.error = null;
    } catch (error: unknown) {
      const e = AppError.from(error);
      this.error = e.message;
      toasts.error(`Could not read the library: ${e.message}`, e.detail);
    } finally {
      this.loading = false;
      this.loaded = true;
    }
  }

  setViewMode(mode: ViewMode): void {
    this.viewMode = mode;
    try {
      localStorage.setItem(VIEW_KEY, mode);
    } catch {
      // Fine without.
    }
  }

  /** Selects a tile: alone, toggled (Ctrl) or as a range from the anchor (Shift). */
  select(id: string, options: { toggle?: boolean; range?: boolean } = {}): void {
    if (options.range && this.anchor !== null) {
      const ids = this.visibleTiles.map((t) => t.id);
      const from = ids.indexOf(this.anchor);
      const to = ids.indexOf(id);
      if (from >= 0 && to >= 0) {
        const [a, b] = from < to ? [from, to] : [to, from];
        this.selected = ids.slice(a, b + 1);
        return;
      }
    }
    if (options.toggle) {
      this.selected = this.selected.includes(id)
        ? this.selected.filter((s) => s !== id)
        : [...this.selected, id];
    } else {
      this.selected = [id];
    }
    this.anchor = id;
  }

  selectAll(): void {
    this.selected = this.visibleTiles.map((t) => t.id);
  }

  /** Asks the detail panel to confirm removing the selection: the selected games, or the
   *  one selected folder. */
  requestRemove(): void {
    if (this.single?.kind === "folder") {
      this.removeFolderPrompt = true;
    } else if (this.selectedGames.length > 0) {
      this.removePrompt = true;
    }
  }

  clearSelection(): void {
    this.selected = [];
    this.anchor = null;
  }

  /** Shows a folder in the grid. */
  openFolder(path: MenuPath): void {
    this.folder = path;
    this.clearSelection();
    this.filter = "";
  }

  goUp(): void {
    if (this.folder.length > 0) {
      this.openFolder(this.folder.slice(0, -1));
    }
  }

  /** The move payload for tiles. */
  itemsFor(ids: string[]): MenuItem[] {
    const byId = new Map(this.tiles.map((t) => [t.id, t]));
    const items: MenuItem[] = [];
    for (const id of ids) {
      const tile = byId.get(id);
      if (!tile) {
        continue;
      }
      if (tile.kind === "folder") {
        items.push({ kind: "folder", path: tile.path });
      } else {
        items.push({ kind: "game", code: tile.kind === "game" ? tile.view.game.code : tile.code });
      }
    }
    return items;
  }

  /** Native open dialog for ROM files. */
  async addGames(): Promise<void> {
    if (!inTauri) {
      toasts.info("Adding games works inside the desktop app only.");
      return;
    }
    const picked = await open({
      multiple: true,
      title: "Add games",
      filters: [{ name: "ROMs and archives", extensions: IMPORT_EXTENSIONS }],
    });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length > 0) {
      await this.importPaths(paths);
    }
  }

  /** Native open dialog for a folder of ROMs. */
  async addFolder(): Promise<void> {
    if (!inTauri) {
      toasts.info("Adding games works inside the desktop app only.");
      return;
    }
    const picked = await open({ directory: true, multiple: true, title: "Add a folder of games" });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length > 0) {
      await this.importPaths(paths);
    }
  }

  /** Imports files and folders into the current folder. */
  async importPaths(paths: string[]): Promise<void> {
    if (this.importing || paths.length === 0) {
      return;
    }
    if (!inTauri) {
      toasts.info("Importing works inside the desktop app only.");
      return;
    }
    this.importing = true;
    this.importOpen = true;
    this.importFiles = [];
    this.importReport = null;
    const folder = this.menu?.custom ? this.folder : null;
    try {
      const report = await libraryImport(paths, folder);
      this.importReport = report;
      this.importFiles = report.files;
      await this.load();
      if (report.codes.length > 0) {
        if (!this.menu?.custom) {
          // An automatic structure decides where the games went: show the first one.
          const first = report.codes[0]!;
          const path = this.menu ? findGame(this.menu.entries, first) : null;
          if (path) {
            this.openFolder(path.slice(0, -1));
          }
        }
        this.selected = report.codes.filter((code) => this.tiles.some((t) => t.id === code));
      }
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      this.importOpen = this.importFiles.length > 0;
    } finally {
      this.importing = false;
    }
  }

  closeImport(): void {
    this.importOpen = false;
  }

  private onImportEvent(event: ImportEvent): void {
    const files = this.importFiles;
    if (files[event.index]) {
      files[event.index] = event;
    } else {
      files.push(event);
    }
  }

  /** Saves a game's fields; returns the game as saved. */
  async save(edit: GameEdit): Promise<GameView | null> {
    if (!inTauri) {
      toasts.info("Saving works inside the desktop app only.");
      return null;
    }
    this.saving = true;
    try {
      const saved = await libraryUpdate(edit);
      this.patchGame(saved);
      return saved;
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      return null;
    } finally {
      this.saving = false;
    }
  }

  /** Removes games from the library. */
  async remove(codes: string[]): Promise<void> {
    if (!inTauri || codes.length === 0) {
      return;
    }
    this.removing = true;
    try {
      const removed = await libraryRemove(codes);
      this.selected = this.selected.filter((id) => !codes.includes(id));
      await this.load();
      toasts.success(removed === 1 ? "1 game removed." : `${removed} games removed.`);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      await this.load();
    } finally {
      this.removing = false;
    }
  }

  /** Fetches art and metadata from TheGamesDB. */
  async fetchArt(codes: string[], force: boolean): Promise<void> {
    if (this.artBusy || codes.length === 0) {
      return;
    }
    if (!inTauri) {
      toasts.info("Fetching art works inside the desktop app only.");
      return;
    }
    this.artBusy = true;
    this.artProgress = null;
    try {
      const outcome = await artFetch(codes, force);
      await this.load();
      toasts.success(outcome.summary);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      await this.load();
    } finally {
      this.artBusy = false;
      this.artProgress = null;
    }
  }

  /** The picture chosen for a folder, when it has one and the tree knows its URL. */
  folderCover(folder: MenuFolder): string | null {
    const image = folder.image;
    if (typeof image === "object" && "custom" in image) {
      return this.menu?.folder_art[image.custom]?.cover ?? null;
    }
    return null;
  }

  /** Native open dialog for a folder picture; the console shows it on the folder's tile. */
  async chooseFolderImage(path: MenuPath): Promise<void> {
    if (!inTauri) {
      toasts.info("Choosing an image works inside the desktop app only.");
      return;
    }
    const picked = await open({
      multiple: false,
      title: "Choose a folder picture",
      filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
    });
    const file = Array.isArray(picked) ? picked[0] : picked;
    if (!file) {
      return;
    }
    if (await this.edit({ op: "set_folder_image", path, file })) {
      toasts.success("Folder picture set.");
    }
  }

  /** Back to the plain folder icon. */
  async clearFolderImage(path: MenuPath): Promise<void> {
    if (await this.edit({ op: "set_folder_image", path, file: null })) {
      toasts.success("Folder picture removed.");
    }
  }

  /** Native open dialog for a cover image. */
  async chooseCover(code: string): Promise<void> {
    if (!inTauri) {
      toasts.info("Choosing an image works inside the desktop app only.");
      return;
    }
    const picked = await open({
      multiple: false,
      title: "Choose a cover image",
      filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
    });
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (!path) {
      return;
    }
    this.coverBusy = true;
    try {
      this.patchGame(await artSetCover(code, path));
      toasts.success("Cover replaced.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.coverBusy = false;
    }
  }

  /** Applies an automatic structure. */
  async setStructure(options: MenuOptions): Promise<void> {
    if (!inTauri) {
      toasts.info("Structures apply inside the desktop app only.");
      return;
    }
    this.menuBusy = true;
    try {
      this.applyMenu(await menuSetStructure(options));
      this.openFolder([]);
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.menuBusy = false;
    }
  }

  /** Moves games and folders into a folder. */
  async move(items: MenuItem[], target: MenuPath, index: number | null = null): Promise<void> {
    if (items.length === 0) {
      return;
    }
    if (!inTauri) {
      toasts.info("Moving works inside the desktop app only.");
      return;
    }
    this.menuBusy = true;
    try {
      this.applyMenu(await menuMove(items, target, index));
      this.clearSelection();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      this.menuBusy = false;
    }
  }

  /** Adds, renames or removes a folder. */
  async edit(edit: MenuEdit): Promise<boolean> {
    if (!inTauri) {
      toasts.info("Editing folders works inside the desktop app only.");
      return false;
    }
    this.menuBusy = true;
    try {
      this.applyMenu(await menuEdit(edit));
      if (edit.op === "remove_folder" && samePath(edit.path, this.folder)) {
        this.openFolder(edit.path.slice(0, -1));
      }
      return true;
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
      return false;
    } finally {
      this.menuBusy = false;
    }
  }

  private applyMenu(menu: MenuView): void {
    this.menu = menu;
    if (entriesAt(menu.entries, this.folder) === null) {
      this.folder = [];
    }
    this.revision += 1;
  }

  private patchGame(view: GameView): void {
    if (!this.view) {
      return;
    }
    const index = this.view.games.findIndex((g) => g.game.code === view.game.code);
    if (index >= 0) {
      this.view.games[index] = view;
    }
    this.revision += 1;
  }
}

function tileName(tile: Tile): string {
  return tile.kind === "game" ? tile.view.game.name : tile.kind === "original" ? tile.name : tile.folder.name;
}

function tileSortKey(tile: Tile): string {
  if (tile.kind === "game") {
    const game = tile.view.game;
    return (game.sort_name.trim() || game.name).toLowerCase();
  }
  return tileName(tile).toLowerCase();
}

/** The path of a game's entry. */
export function findGame(entries: MenuEntry[], code: string, prefix: MenuPath = []): MenuPath | null {
  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index]!;
    if ((entry.kind === "game" || entry.kind === "original") && entry.code === code) {
      return [...prefix, index];
    }
    if (entry.kind === "folder") {
      const found = findGame(entry.entries, code, [...prefix, index]);
      if (found) {
        return found;
      }
    }
  }
  return null;
}

export const libraryStore = new LibraryStore();
