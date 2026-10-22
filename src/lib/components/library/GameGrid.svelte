<script lang="ts">
  import { tick } from "svelte";
  import Icon from "../Icon.svelte";
  import type { MenuItem, MenuPath } from "../../api/types";
  import { DRAG_TYPE, breadcrumbs, formatBytes, libraryStore, type Tile } from "../../state";

  const tiles = $derived(libraryStore.visibleTiles);
  const crumbs = $derived(libraryStore.menu ? breadcrumbs(libraryStore.menu.entries, libraryStore.folder) : []);
  const selectedSet = $derived(new Set(libraryStore.selected));
  const summary = $derived.by(() => {
    const all = libraryStore.tiles;
    const games = all.filter((t) => t.kind !== "folder").length;
    const folders = all.length - games;
    const parts = [`${games} game${games === 1 ? "" : "s"}`];
    if (folders > 0) {
      parts.push(`${folders} folder${folders === 1 ? "" : "s"}`);
    }
    if (libraryStore.selected.length > 0) {
      parts.push(`${libraryStore.selected.length} selected`);
    }
    return parts.join(" · ");
  });

  /* The cover grid is windowed: only the rows in view (plus a margin) exist in the DOM, so a
     library of hundreds of games scrolls as one of a dozen. The tile width follows the pane,
     the row height is measured from the first rendered tile. The list view is a plain table. */
  const GAP = 12;
  const PAD = 12;
  const MIN_TILE = 128;
  const MAX_TILE = 200;
  const OVERSCAN_ROWS = 2;

  let viewport: HTMLElement | undefined = $state();
  let viewportWidth = $state(0);
  let viewportHeight = $state(0);
  let scrollTop = $state(0);
  let measuredTile = $state(0);

  const cols = $derived(
    libraryStore.viewMode === "grid"
      ? Math.max(1, Math.floor((viewportWidth - PAD * 2 + GAP) / (MIN_TILE + GAP)))
      : 1,
  );
  const tileWidth = $derived(
    Math.min(MAX_TILE, Math.floor((viewportWidth - PAD * 2 - GAP * (cols - 1)) / Math.max(1, cols))),
  );
  /** Art at 228:204 inside the tile padding, two title lines and a subtitle. */
  const tileHeight = $derived(measuredTile || Math.round((tileWidth - 16) * (204 / 228) + 66));
  const rowHeight = $derived(tileHeight + GAP);
  const rows = $derived(Math.ceil(tiles.length / cols));
  const totalHeight = $derived(rows > 0 ? rows * rowHeight - GAP + PAD * 2 : 0);
  const firstRow = $derived(Math.max(0, Math.floor((scrollTop - PAD) / rowHeight) - OVERSCAN_ROWS));
  const lastRow = $derived(
    Math.min(rows - 1, Math.ceil((scrollTop + viewportHeight - PAD) / rowHeight) + OVERSCAN_ROWS),
  );
  const windowStart = $derived(firstRow * cols);
  const visible = $derived(
    libraryStore.viewMode === "grid" ? tiles.slice(windowStart, (lastRow + 1) * cols) : tiles,
  );
  const offsetY = $derived(PAD + firstRow * rowHeight);

  $effect(() => {
    if (!viewport) {
      return;
    }
    const node = viewport;
    const observer = new ResizeObserver(() => {
      viewportWidth = node.clientWidth;
      viewportHeight = node.clientHeight;
    });
    observer.observe(node);
    viewportWidth = node.clientWidth;
    viewportHeight = node.clientHeight;
    return () => observer.disconnect();
  });

  /** Measures the first rendered tile; every tile has the same height. */
  function measure(node: HTMLElement, first: boolean): { update: (first: boolean) => void; destroy: () => void } {
    let observer: ResizeObserver | null = null;
    const apply = (on: boolean) => {
      if (on && !observer) {
        observer = new ResizeObserver(() => {
          if (node.offsetHeight > 0) {
            measuredTile = node.offsetHeight;
          }
        });
        observer.observe(node);
      } else if (!on && observer) {
        observer.disconnect();
        observer = null;
      }
    };
    apply(first);
    return { update: apply, destroy: () => observer?.disconnect() };
  }

  function onscroll(): void {
    if (viewport) {
      scrollTop = viewport.scrollTop;
    }
  }

  let focusId = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);

  /** The one tab stop: the focused tile when it is rendered, else the first rendered one. */
  const tabTarget = $derived(
    focusId && visible.some((t) => t.id === focusId) ? focusId : (visible[0]?.id ?? null),
  );

  /** Keeps keyboard handling alive when the focused tile scrolls out of the window. */
  function keepFocus(node: HTMLElement): { destroy: () => void } {
    return {
      destroy: () => {
        if (document.activeElement === node) {
          viewport?.focus({ preventScroll: true });
        }
      },
    };
  }

  function title(tile: Tile): string {
    return tile.kind === "game" ? tile.view.game.name : tile.kind === "original" ? tile.name : tile.folder.name;
  }

  function subtitle(tile: Tile): string {
    if (tile.kind === "game") {
      const year = tile.view.game.release_date.startsWith("1900") ? "" : tile.view.game.release_date.slice(0, 4);
      return [tile.view.system_name, year].filter(Boolean).join(" · ");
    }
    if (tile.kind === "original") {
      return "Built in";
    }
    return `${tile.games} game${tile.games === 1 ? "" : "s"}`;
  }

  /** The accessible name of a tile: what it shows, without the remove button's label. */
  function tileLabel(tile: Tile): string {
    if (tile.kind === "game") {
      const game = tile.view.game;
      return [game.name, tile.view.system_name, game.release_date?.slice(0, 4)].filter(Boolean).join(", ");
    }
    if (tile.kind === "original") {
      return `${tile.name}, built in`;
    }
    return `${tile.folder.name}, folder, ${tile.games} ${tile.games === 1 ? "game" : "games"}`;
  }

  function onTileClick(event: MouseEvent, tile: Tile): void {
    libraryStore.select(tile.id, { toggle: event.ctrlKey || event.metaKey, range: event.shiftKey });
    focusId = tile.id;
  }

  function onTileDblClick(tile: Tile): void {
    if (tile.kind === "folder") {
      libraryStore.openFolder(tile.path);
    }
  }

  /** Scrolls the tile's row into the window, waits for it to render, then focuses it. */
  async function focusTile(id: string): Promise<void> {
    focusId = id;
    if (!viewport) {
      return;
    }
    if (libraryStore.viewMode === "grid") {
      const index = tiles.findIndex((t) => t.id === id);
      if (index < 0) {
        return;
      }
      const top = PAD + Math.floor(index / cols) * rowHeight;
      const bottom = top + tileHeight;
      if (top < viewport.scrollTop) {
        viewport.scrollTop = top - PAD;
      } else if (bottom > viewport.scrollTop + viewport.clientHeight) {
        viewport.scrollTop = bottom + PAD - viewport.clientHeight;
      }
      scrollTop = viewport.scrollTop;
      await tick();
    }
    const node = viewport.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"]`);
    node?.focus({ preventScroll: true });
    node?.scrollIntoView({ block: "nearest" });
  }

  function onGridKey(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && (event.key === "a" || event.key === "A")) {
      event.preventDefault();
      libraryStore.selectAll();
      return;
    }
    if (event.key === "Escape") {
      libraryStore.clearSelection();
      return;
    }
    if (event.key === "Backspace" && libraryStore.folder.length > 0) {
      event.preventDefault();
      libraryStore.goUp();
      return;
    }
    if (event.key === "Delete") {
      event.preventDefault();
      libraryStore.requestRemove();
      return;
    }
    const index = tiles.findIndex((t) => t.id === focusId);
    const current = tiles[index];
    if (!current) {
      return;
    }
    if (event.key === "Enter") {
      onTileDblClick(current);
      return;
    }
    if (event.key === " ") {
      event.preventDefault();
      libraryStore.select(current.id, { toggle: true });
      return;
    }
    let next = index;
    switch (event.key) {
      case "ArrowRight":
        next = Math.min(tiles.length - 1, index + 1);
        break;
      case "ArrowLeft":
        next = Math.max(0, index - 1);
        break;
      case "ArrowDown":
        next = Math.min(tiles.length - 1, index + cols);
        break;
      case "ArrowUp":
        next = Math.max(0, index - cols);
        break;
      case "PageDown":
        next = Math.min(tiles.length - 1, index + cols * Math.max(1, Math.floor(viewportHeight / rowHeight)));
        break;
      case "PageUp":
        next = Math.max(0, index - cols * Math.max(1, Math.floor(viewportHeight / rowHeight)));
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = tiles.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    const target = tiles[next];
    if (target) {
      libraryStore.select(target.id, { range: event.shiftKey });
      void focusTile(target.id);
    }
  }

  function ondragstart(event: DragEvent, tile: Tile): void {
    if (!event.dataTransfer) {
      return;
    }
    if (!selectedSet.has(tile.id)) {
      libraryStore.select(tile.id);
    }
    const ids = libraryStore.selected.includes(tile.id) ? libraryStore.selected : [tile.id];
    event.dataTransfer.setData(DRAG_TYPE, JSON.stringify(libraryStore.itemsFor(ids)));
    event.dataTransfer.effectAllowed = "move";
  }

  function accepts(event: DragEvent): boolean {
    return event.dataTransfer?.types.includes(DRAG_TYPE) ?? false;
  }

  function onFolderDragOver(event: DragEvent, id: string): void {
    if (accepts(event)) {
      event.preventDefault();
      event.dataTransfer!.dropEffect = "move";
      dropTarget = id;
    }
  }

  function onFolderDrop(event: DragEvent, path: MenuPath, id: string): void {
    dropTarget = null;
    const raw = event.dataTransfer?.getData(DRAG_TYPE);
    if (!raw) {
      return;
    }
    event.preventDefault();
    const items = JSON.parse(raw) as MenuItem[];
    // Dropping a folder onto itself is a no-op, not an error.
    const self = items.some((i) => i.kind === "folder" && `f:${i.path.join("/")}` === id);
    if (!self) {
      void libraryStore.move(items, path);
    }
  }
</script>

<div class="pane">
  <div class="toolbar">
    <nav class="crumbs" aria-label="Folder path">
      {#each crumbs as crumb, i (crumb.path.join("/"))}
        {#if i > 0}<span class="sep" aria-hidden="true">›</span>{/if}
        <button
          type="button"
          class="crumb"
          class:current={i === crumbs.length - 1}
          class:dropping={dropTarget === `crumb:${i}`}
          aria-current={i === crumbs.length - 1 ? "location" : undefined}
          onclick={() => libraryStore.openFolder(crumb.path)}
          ondragover={(e) => onFolderDragOver(e, `crumb:${i}`)}
          ondragleave={() => (dropTarget = null)}
          ondrop={(e) => onFolderDrop(e, crumb.path, `crumb:${i}`)}
        >
          {crumb.name}
        </button>
      {/each}
    </nav>
    <span class="summary muted num">{summary}</span>
    <span class="spacer"></span>
    <label class="filter">
      <Icon name="search" size={14} />
      <input type="search" placeholder="Filter" aria-label="Filter the games in this folder" bind:value={libraryStore.filter} />
    </label>
    <div class="view-toggle" role="group" aria-label="View">
      <button type="button" class="icon-btn icon-btn-sm" aria-pressed={libraryStore.viewMode === "grid"} aria-label="Covers" title="Covers" onclick={() => libraryStore.setViewMode("grid")}>
        <Icon name="grid" size={16} />
      </button>
      <button type="button" class="icon-btn icon-btn-sm" aria-pressed={libraryStore.viewMode === "list"} aria-label="List" title="List" onclick={() => libraryStore.setViewMode("list")}>
        <Icon name="list" size={16} />
      </button>
    </div>
    <button type="button" class="btn" onclick={() => libraryStore.addGames()} disabled={libraryStore.importing}>
      <Icon name="plus" size={16} />
      <span>Add games…</span>
    </button>
    <button type="button" class="btn btn-quiet" onclick={() => libraryStore.addFolder()} disabled={libraryStore.importing}>
      Add a folder…
    </button>
  </div>

  {#if tiles.length === 0}
    <div class="empty-note">
      {#if libraryStore.filter.trim()}
        <p>Nothing in this folder matches “{libraryStore.filter.trim()}”.</p>
        <button type="button" class="btn" onclick={() => (libraryStore.filter = "")}>Clear the filter</button>
      {:else}
        <p>This folder is empty. Drag games here from another folder, or add some.</p>
        <button type="button" class="btn" onclick={() => libraryStore.addGames()} disabled={libraryStore.importing}>
          <Icon name="plus" size={16} />
          <span>Add games…</span>
        </button>
      {/if}
    </div>
  {:else if libraryStore.viewMode === "grid"}
    <div
      class="viewport focus-inset"
      role="listbox"
      tabindex="-1"
      aria-multiselectable="true"
      aria-label="Games in this folder"
      bind:this={viewport}
      {onscroll}
      onkeydown={onGridKey}
    >
      <div class="sizer" style:height="{totalHeight}px">
        <div
          class="window"
          style:transform="translateY({offsetY}px)"
          style:grid-template-columns="repeat({cols}, {tileWidth}px)"
        >
          {#each visible as tile, i (tile.id)}
            <!-- Keys are handled once, on the listbox, so they keep working when the focused
                 tile scrolls out of the window and focus falls back to the listbox. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              role="option"
              class="tile kind-{tile.kind} focus-inset"
              class:selected={selectedSet.has(tile.id)}
              class:dropping={dropTarget === tile.id}
              data-id={tile.id}
              tabindex={tile.id === tabTarget ? 0 : -1}
              aria-selected={selectedSet.has(tile.id)}
              aria-label={tileLabel(tile)}
              aria-posinset={windowStart + i + 1}
              aria-setsize={tiles.length}
              draggable="true"
              use:keepFocus
              use:measure={i === 0}
              onclick={(e) => onTileClick(e, tile)}
              ondblclick={() => onTileDblClick(tile)}
              onfocus={() => (focusId = tile.id)}
              ondragstart={(e) => ondragstart(e, tile)}
              ondragover={(e) => (tile.kind === "folder" ? onFolderDragOver(e, tile.id) : undefined)}
              ondragleave={() => (dropTarget = null)}
              ondrop={(e) => (tile.kind === "folder" ? onFolderDrop(e, tile.path, tile.id) : undefined)}
            >
              <div class="art">
                {#if tile.kind === "game"}
                  <img src={tile.view.cover} alt="" loading="lazy" decoding="async" draggable="false" crossorigin="anonymous" />
                  {#if tile.view.game.placeholder_art && !selectedSet.has(tile.id)}
                    <span class="noart">No art yet</span>
                  {/if}
                {:else if tile.kind === "original"}
                  {#if tile.cover}
                    <img src={tile.cover} alt="" loading="lazy" decoding="async" draggable="false" crossorigin="anonymous" />
                  {:else}
                    <span class="builtin"><Icon name="builtIn" size={32} /></span>
                  {/if}
                {:else if tile.cover}
                  <img src={tile.cover} alt="" loading="lazy" decoding="async" draggable="false" crossorigin="anonymous" />
                {:else}
                  <span class="folder-art">
                    <Icon name="folder" size={40} />
                  </span>
                {/if}
              </div>
              <span class="title">{title(tile)}</span>
              <span class="sub muted">{subtitle(tile)}</span>
              {#if tile.kind === "game" && selectedSet.has(tile.id)}
                <button
                  type="button"
                  class="tile-remove"
                  aria-label="Remove {tile.view.game.name} from the library"
                  title="Remove from the library (Delete)"
                  tabindex="-1"
                  onclick={(e) => { e.stopPropagation(); libraryStore.removePrompt = true; }}
                  ondblclick={(e) => e.stopPropagation()}
                  onpointerdown={(e) => e.stopPropagation()}
                >
                  <Icon name="trash" size={14} />
                </button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    </div>
  {:else}
    <div class="list-wrap" bind:this={viewport}>
      <table class="list" role="grid" aria-multiselectable="true" aria-label="Games in this folder" onkeydown={onGridKey}>
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col">System</th>
            <th scope="col">Publisher</th>
            <th scope="col">Year</th>
            <th scope="col" class="num">Players</th>
            <th scope="col" class="num">Size</th>
            <th scope="col">Art</th>
          </tr>
        </thead>
        <tbody>
          {#each tiles as tile (tile.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <tr
              class="focus-inset"
              class:selected={selectedSet.has(tile.id)}
              class:dropping={dropTarget === tile.id}
              data-id={tile.id}
              tabindex={tile.id === tabTarget ? 0 : -1}
              aria-selected={selectedSet.has(tile.id)}
              draggable="true"
              onclick={(e) => onTileClick(e, tile)}
              ondblclick={() => onTileDblClick(tile)}
              onfocus={() => (focusId = tile.id)}
              ondragstart={(e) => ondragstart(e, tile)}
              ondragover={(e) => (tile.kind === "folder" ? onFolderDragOver(e, tile.id) : undefined)}
              ondragleave={() => (dropTarget = null)}
              ondrop={(e) => (tile.kind === "folder" ? onFolderDrop(e, tile.path, tile.id) : undefined)}
            >
              <td class="name-cell">
                {#if tile.kind === "game"}
                  <img class="thumb" src={tile.view.thumbnail} alt="" loading="lazy" decoding="async" draggable="false" crossorigin="anonymous" />
                {:else if tile.kind === "original" && tile.thumbnail}
                  <img class="thumb" src={tile.thumbnail} alt="" loading="lazy" decoding="async" draggable="false" crossorigin="anonymous" />
                {:else}
                  <span class="thumb glyph"><Icon name={tile.kind === "folder" ? "folder" : "builtIn"} size={18} /></span>
                {/if}
                <span>{title(tile)}</span>
              </td>
              {#if tile.kind === "game"}
                <td>{tile.view.system_name}</td>
                <td>{tile.view.game.publisher === "UNKNOWN" ? "" : tile.view.game.publisher}</td>
                <td class="num">{tile.view.game.release_date.startsWith("1900") ? "" : tile.view.game.release_date.slice(0, 4)}</td>
                <td class="num">{tile.view.game.players}</td>
                <td class="num">{formatBytes(tile.view.size)}</td>
                <td>{tile.view.game.placeholder_art ? "none" : "yes"}</td>
              {:else if tile.kind === "original"}
                <td colspan="6" class="muted">Built into the console</td>
              {:else}
                <td colspan="6" class="muted">{subtitle(tile)}</td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }

  .crumb {
    min-height: var(--control-sm);
    padding: 0 var(--space-1);
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    white-space: nowrap;
  }

  .crumb:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .crumb.current {
    color: var(--text);
    font-weight: var(--weight-semibold);
  }

  .crumb.dropping {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .sep {
    color: var(--text-faint);
  }

  .summary {
    font-size: var(--text-sm);
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .filter {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--text-muted);
  }

  .filter input {
    width: 144px;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
  }

  .view-toggle .icon-btn[aria-pressed="true"] {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .viewport {
    flex: 1;
    min-height: 0;
    overflow: auto;
    position: relative;
  }

  .sizer {
    position: relative;
    width: 100%;
  }

  .window {
    position: absolute;
    left: 12px;
    right: 12px;
    top: 0;
    display: grid;
    gap: 12px;
    will-change: transform;
  }

  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-m);
    cursor: default;
    user-select: none;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      border-color var(--motion-fast) var(--ease-out);
  }

  .tile:hover {
    background: var(--surface-hover);
  }

  .tile.selected {
    background: var(--accent-soft);
    border-color: var(--accent);
  }

  .tile.dropping {
    border-color: var(--accent);
    border-style: dashed;
  }

  .art {
    position: relative;
    aspect-ratio: 228 / 204;
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--surface-raised);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-faint);
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .noart {
    position: absolute;
    bottom: var(--space-1);
    right: var(--space-1);
    padding: 1px var(--space-2);
    border-radius: 999px;
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  .tile-remove {
    position: absolute;
    right: var(--space-2);
    bottom: var(--space-2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--text-muted);
    cursor: pointer;
    box-shadow: var(--shadow-float);
  }

  .tile-remove:hover,
  .tile-remove:focus-visible {
    background: var(--danger-soft);
    border-color: var(--danger);
    color: var(--danger);
  }

  .builtin,
  .folder-art {
    display: inline-flex;
    color: var(--text-faint);
  }

  .kind-folder .art {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .title {
    font-size: var(--text-sm);
    line-height: var(--leading-tight);
    height: calc(2 * var(--text-sm) * var(--leading-tight));
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .sub {
    font-size: var(--text-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Room for the remove button in the corner of a selected game tile. */
  .tile.selected.kind-game .sub {
    padding-right: 32px;
  }

  .list-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .list {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-md);
  }

  .list th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--surface);
    text-align: left;
    font-weight: var(--weight-semibold);
    color: var(--text-muted);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--border);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .list th.num,
  .list td.num {
    text-align: right;
  }

  .list td {
    padding: var(--space-1) var(--space-3);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  .list tbody tr {
    cursor: default;
  }

  .list tbody tr:hover {
    background: var(--surface-hover);
  }

  .list tr.selected {
    background: var(--accent-soft);
  }

  .list tr.dropping td {
    border-bottom-color: var(--accent);
  }

  .name-cell {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .thumb {
    width: 28px;
    height: 24px;
    object-fit: contain;
    border-radius: 3px;
    background: var(--surface-raised);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-faint);
  }
</style>
