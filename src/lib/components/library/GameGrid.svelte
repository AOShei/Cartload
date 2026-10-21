<script lang="ts">
  import Icon from "../Icon.svelte";
  import type { MenuPath } from "../../api/types";
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

  let grid: HTMLElement | undefined = $state();
  let focusId = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);

  const tabTarget = $derived(focusId && tiles.some((t) => t.id === focusId) ? focusId : (tiles[0]?.id ?? null));

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

  function onTileClick(event: MouseEvent, tile: Tile): void {
    libraryStore.select(tile.id, { toggle: event.ctrlKey || event.metaKey, range: event.shiftKey });
    focusId = tile.id;
  }

  function onTileDblClick(tile: Tile): void {
    if (tile.kind === "folder") {
      libraryStore.openFolder(tile.path);
    }
  }

  function focusTile(id: string): void {
    focusId = id;
    grid?.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"]`)?.focus();
  }

  function columns(): number {
    if (!grid || libraryStore.viewMode !== "grid") {
      return 1;
    }
    const first = grid.querySelector<HTMLElement>("[role='option']");
    if (!first) {
      return 1;
    }
    return Math.max(1, Math.floor(grid.clientWidth / first.offsetWidth));
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
    const cols = columns();
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
      focusTile(target.id);
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
    const items = JSON.parse(raw) as import("../../api/types").MenuItem[];
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
      <button type="button" class="icon-btn" aria-pressed={libraryStore.viewMode === "grid"} aria-label="Covers" title="Covers" onclick={() => libraryStore.setViewMode("grid")}>
        <Icon name="grid" size={16} />
      </button>
      <button type="button" class="icon-btn" aria-pressed={libraryStore.viewMode === "list"} aria-label="List" title="List" onclick={() => libraryStore.setViewMode("list")}>
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
    <div class="nothing">
      {#if libraryStore.filter.trim()}
        <p class="muted">Nothing in this folder matches “{libraryStore.filter.trim()}”.</p>
      {:else}
        <p class="muted">This folder is empty. Drag games here from another folder, or add games.</p>
      {/if}
    </div>
  {:else if libraryStore.viewMode === "grid"}
    <div class="tiles" role="listbox" tabindex="-1" aria-multiselectable="true" aria-label="Games in this folder" bind:this={grid}>
      {#each tiles as tile (tile.id)}
        <div
          role="option"
          class="tile kind-{tile.kind}"
          class:selected={selectedSet.has(tile.id)}
          class:dropping={dropTarget === tile.id}
          data-id={tile.id}
          tabindex={tile.id === tabTarget ? 0 : -1}
          aria-selected={selectedSet.has(tile.id)}
          draggable="true"
          onclick={(e) => onTileClick(e, tile)}
          ondblclick={() => onTileDblClick(tile)}
          onkeydown={onGridKey}
          onfocus={() => (focusId = tile.id)}
          ondragstart={(e) => ondragstart(e, tile)}
          ondragover={(e) => (tile.kind === "folder" ? onFolderDragOver(e, tile.id) : undefined)}
          ondragleave={() => (dropTarget = null)}
          ondrop={(e) => (tile.kind === "folder" ? onFolderDrop(e, tile.path, tile.id) : undefined)}
        >
          <div class="art">
            {#if tile.kind === "game"}
              <img src={tile.view.cover} alt="" loading="lazy" decoding="async" draggable="false" />
              {#if tile.view.game.placeholder_art}
                <span class="noart">No art yet</span>
              {/if}
            {:else if tile.kind === "original"}
              <span class="builtin"><Icon name="builtIn" size={32} /></span>
            {:else}
              <span class="folder-art">
                <Icon name="folder" size={40} />
              </span>
            {/if}
          </div>
          <span class="title">{title(tile)}</span>
          <span class="sub muted">{subtitle(tile)}</span>
        </div>
      {/each}
    </div>
  {:else}
    <div class="list-wrap" bind:this={grid}>
      <table class="list" role="grid" aria-multiselectable="true" aria-label="Games in this folder">
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
            <tr
              class:selected={selectedSet.has(tile.id)}
              class:dropping={dropTarget === tile.id}
              data-id={tile.id}
              tabindex={tile.id === tabTarget ? 0 : -1}
              aria-selected={selectedSet.has(tile.id)}
              draggable="true"
              onclick={(e) => onTileClick(e, tile)}
              ondblclick={() => onTileDblClick(tile)}
              onkeydown={onGridKey}
              onfocus={() => (focusId = tile.id)}
              ondragstart={(e) => ondragstart(e, tile)}
              ondragover={(e) => (tile.kind === "folder" ? onFolderDragOver(e, tile.id) : undefined)}
              ondragleave={() => (dropTarget = null)}
              ondrop={(e) => (tile.kind === "folder" ? onFolderDrop(e, tile.path, tile.id) : undefined)}
            >
              <td class="name-cell">
                {#if tile.kind === "game"}
                  <img class="thumb" src={tile.view.thumbnail} alt="" loading="lazy" draggable="false" />
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
    padding: 0.2rem 0.4rem;
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
    font-weight: 600;
  }

  .crumb.dropping {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .sep {
    color: var(--text-faint);
  }

  .summary {
    font-size: 0.8rem;
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
    width: 9rem;
    padding: 0.3rem 0.5rem;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
  }

  .view-toggle .icon-btn {
    width: 1.7rem;
    height: 1.7rem;
  }

  .view-toggle .icon-btn[aria-pressed="true"] {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .nothing {
    padding: var(--space-6);
    text-align: center;
  }

  .tiles {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: var(--space-3);
    padding: var(--space-3);
    align-content: start;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-m);
    cursor: default;
    user-select: none;
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
    bottom: 4px;
    right: 4px;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    color: var(--text-muted);
    font-size: 0.65rem;
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
    font-size: 0.8rem;
    line-height: 1.25;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .sub {
    font-size: 0.7rem;
  }

  .list-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .list {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .list th {
    position: sticky;
    top: 0;
    background: var(--surface);
    text-align: left;
    font-weight: 600;
    color: var(--text-muted);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--border);
    font-size: 0.75rem;
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
