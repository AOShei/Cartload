<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import Icon from "../Icon.svelte";
  import TreeNode from "./TreeNode.svelte";
  import type { MenuOptions, Structure } from "../../api/types";
  import { libraryStore, pathKey } from "../../state";

  const menu = $derived(libraryStore.menu);
  const current = $derived(libraryStore.currentFolder);
  const expanded = new SvelteSet<string>();
  let tree: HTMLElement | undefined = $state();

  /* Structure popover */
  let structureOpen = $state(false);
  let popover: HTMLDetailsElement | undefined = $state();
  let kind = $state<Structure["kind"]>("count");
  let max = $state(30);
  let originalsAtRoot = $state(true);

  $effect(() => {
    const options = menu?.options;
    if (options) {
      kind = options.structure.kind;
      max = options.structure.kind === "count" ? options.structure.max : 30;
      originalsAtRoot = options.originals_at_root;
    }
  });

  function describe(options: MenuOptions | undefined, custom: boolean): string {
    if (custom) {
      return "Folders edited by hand";
    }
    switch (options?.structure.kind) {
      case "flat":
        return "One list";
      case "letters":
        return "Folders A to Z";
      case "count":
        return `Folders of ${options.structure.max} games`;
      default:
        return "Automatic folders";
    }
  }

  async function applyStructure(): Promise<void> {
    const structure: Structure =
      kind === "count" ? { kind, max: Math.max(1, Math.floor(max || 1)) } : { kind };
    await libraryStore.setStructure({ structure, originals_at_root: originalsAtRoot });
    structureOpen = false;
  }

  /* Folder editing */
  let creating = $state(false);
  let newName = $state("");
  let renaming = $state(false);
  let renameValue = $state("");
  let confirmingRemove = $state(false);

  function startCreate(): void {
    creating = true;
    renaming = false;
    confirmingRemove = false;
    newName = "";
  }

  async function commitCreate(): Promise<void> {
    if (!newName.trim()) {
      creating = false;
      return;
    }
    const parent = libraryStore.folder;
    if (await libraryStore.edit({ op: "add_folder", parent, name: newName })) {
      expanded.add(pathKey(parent));
      creating = false;
      newName = "";
    }
  }

  function startRename(): void {
    if (!current) {
      return;
    }
    renameValue = current.name;
    renaming = true;
    creating = false;
    confirmingRemove = false;
  }

  async function commitRename(): Promise<void> {
    if (!renaming) {
      return;
    }
    if (!renameValue.trim() || renameValue === current?.name) {
      renaming = false;
      return;
    }
    if (await libraryStore.edit({ op: "rename_folder", path: libraryStore.folder, name: renameValue })) {
      renaming = false;
    }
  }

  async function removeFolder(): Promise<void> {
    await libraryStore.edit({ op: "remove_folder", path: libraryStore.folder });
    confirmingRemove = false;
  }

  /** Up and Down move between the visible folders, Right expands, Left collapses or goes up. */
  function onkeydown(event: KeyboardEvent): void {
    if (!tree) {
      return;
    }
    const rows = Array.from(tree.querySelectorAll<HTMLElement>("[data-tree-row]"));
    const index = rows.findIndex((row) => row.contains(document.activeElement));
    if (index < 0) {
      return;
    }
    const row = rows[index]!;
    const key = row.dataset["path"] ?? "";
    const focusRow = (target: HTMLElement | undefined) =>
      target?.querySelector<HTMLElement>(".label")?.focus();
    switch (event.key) {
      case "ArrowDown":
        focusRow(rows[index + 1]);
        break;
      case "ArrowUp":
        focusRow(rows[index - 1]);
        break;
      case "Home":
        focusRow(rows[0]);
        break;
      case "End":
        focusRow(rows[rows.length - 1]);
        break;
      case "ArrowRight":
        if (key) {
          expanded.add(key);
        }
        break;
      case "ArrowLeft":
        if (expanded.has(key)) {
          expanded.delete(key);
        } else {
          const parentKey = key.split("/").slice(0, -1).join("/");
          focusRow(rows.find((r) => (r.dataset["path"] ?? "") === parentKey));
        }
        break;
      case "F2":
        startRename();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  /** Focuses a field when it appears (inline rename and new-folder boxes). */
  function autofocus(node: HTMLElement): void {
    node.focus();
  }

  function onwindowclick(event: MouseEvent): void {
    if (structureOpen && popover && !popover.contains(event.target as Node)) {
      structureOpen = false;
    }
  }

  function onwindowkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      structureOpen = false;
      creating = false;
      renaming = false;
      confirmingRemove = false;
    }
  }
</script>

<svelte:window onclick={onwindowclick} onkeydown={onwindowkeydown} />

<div class="pane">
  <header>
    <h3>Folders</h3>
    <div class="tools">
      <button type="button" class="icon-btn" aria-label="New folder" title="New folder in the selected folder" onclick={startCreate}>
        <Icon name="folderPlus" size={18} />
      </button>
      <details class="popover" bind:this={popover} bind:open={structureOpen}>
        <summary class="icon-btn" aria-label="Automatic folder structure" title="Automatic folder structure">
          <Icon name="settings" size={18} />
        </summary>
        <div class="popover-body">
          <h4>Folder structure</h4>
          <p class="muted">How the console groups the games. Applying replaces the folders.</p>
          <fieldset>
            <legend class="visually-hidden">Structure</legend>
            <label><input type="radio" name="structure" value="flat" bind:group={kind} /> One list</label>
            <label><input type="radio" name="structure" value="letters" bind:group={kind} /> Folders A to Z</label>
            <label>
              <input type="radio" name="structure" value="count" bind:group={kind} />
              Folders of
              <input type="number" class="num small" min="1" max="999" bind:value={max} aria-label="Games per folder" disabled={kind !== "count"} />
              games
            </label>
          </fieldset>
          <label class="check">
            <input type="checkbox" bind:checked={originalsAtRoot} />
            <span>Keep the console's own games in the main menu, with the library in “More games…”</span>
          </label>
          {#if menu?.custom}
            <p class="note">You have arranged the folders by hand. Applying a structure replaces them.</p>
          {/if}
          <div class="popover-actions">
            <button type="button" class="btn" onclick={applyStructure}>Apply</button>
            <button type="button" class="btn btn-quiet" onclick={() => (structureOpen = false)}>Close</button>
          </div>
        </div>
      </details>
    </div>
  </header>
  <p class="structure muted">{describe(menu?.options, menu?.custom ?? false)}</p>

  {#if menu}
    <ul class="tree" role="tree" aria-label="Console folders" bind:this={tree} {onkeydown}>
      <TreeNode path={[]} name="Console menu" entries={menu.entries} depth={0} {expanded} />
    </ul>
  {:else}
    <p class="muted loading">Loading…</p>
  {/if}

  <footer>
    {#if creating}
      <form class="inline" onsubmit={(e) => { e.preventDefault(); void commitCreate(); }}>
        <input type="text" placeholder="Folder name" bind:value={newName} aria-label="New folder name" use:autofocus />
        <button type="submit" class="btn">Add</button>
        <button type="button" class="btn btn-quiet" onclick={() => (creating = false)}>Cancel</button>
      </form>
    {:else if current}
      {#if renaming}
        <form class="inline" onsubmit={(e) => { e.preventDefault(); void commitRename(); }}>
          <input type="text" bind:value={renameValue} aria-label="Folder name" use:autofocus onblur={commitRename} />
          <button type="submit" class="btn">Rename</button>
        </form>
      {:else if confirmingRemove}
        <p class="confirm">Remove “{current.name}”? Its games move up one level.</p>
        <div class="inline">
          <button type="button" class="btn danger" onclick={removeFolder}>Remove folder</button>
          <button type="button" class="btn btn-quiet" onclick={() => (confirmingRemove = false)}>Keep</button>
        </div>
      {:else}
        <div class="inline">
          <span class="folder-name">{current.name}</span>
          <button type="button" class="icon-btn" aria-label="Rename folder" title="Rename (F2)" onclick={startRename}>
            <Icon name="edit" size={16} />
          </button>
          <button type="button" class="icon-btn" aria-label="Remove folder" title="Remove folder" onclick={() => (confirmingRemove = true)}>
            <Icon name="trash" size={16} />
          </button>
        </div>
      {/if}
    {:else}
      <p class="muted hint">Drag games or folders onto a folder to move them.</p>
    {/if}
  </footer>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-3) 0;
  }

  h3 {
    margin: 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .tools {
    display: flex;
    gap: var(--space-1);
    align-items: center;
  }

  .structure {
    padding: 0 var(--space-3);
    font-size: 0.8rem;
  }

  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: var(--space-2) 0 0;
    padding: 0 var(--space-2);
  }

  .loading {
    padding: var(--space-3);
  }

  footer {
    border-top: 1px solid var(--border);
    padding: var(--space-2) var(--space-3);
    font-size: 0.85rem;
    min-height: 3rem;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .inline input[type="text"] {
    flex: 1;
    min-width: 0;
  }

  .folder-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .confirm {
    margin-bottom: var(--space-2);
  }

  .hint {
    font-size: 0.8rem;
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
  }

  .popover {
    position: relative;
  }

  .popover > summary {
    list-style: none;
  }

  .popover > summary::-webkit-details-marker {
    display: none;
  }

  .popover-body {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 20;
    width: 20rem;
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    box-shadow: var(--shadow-float);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: 0.9rem;
  }

  .popover-body h4 {
    margin: 0;
    font-size: 0.95rem;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    border: 0;
  }

  fieldset label,
  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .check {
    align-items: flex-start;
  }

  .small {
    width: 4rem;
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--surface);
  }

  .note {
    color: var(--warn);
  }

  .popover-actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }
</style>
