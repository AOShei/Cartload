<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import TreeNode from "./TreeNode.svelte";
  import type { Structure } from "../../api/types";
  import { libraryStore, pathKey } from "../../state";

  const menu = $derived(libraryStore.menu);
  const expanded = new SvelteSet<string>();
  let tree: HTMLElement | undefined = $state();

  /** The folder being viewed is always visible in the tree, so it is the tree's tab stop. */
  $effect(() => {
    const path = libraryStore.folder;
    for (let depth = 1; depth < path.length; depth += 1) {
      expanded.add(pathKey(path.slice(0, depth)));
    }
  });

  /* Structure popover */
  let structureOpen = $state(false);
  let kind = $state<Structure["kind"]>("count");
  let max = $state(30);
  let originalsAtRoot = $state(true);
  let confirmApply = $state(false);

  $effect(() => {
    const options = menu?.options;
    if (options) {
      kind = options.structure.kind;
      max = options.structure.kind === "count" ? options.structure.max : 30;
      originalsAtRoot = options.originals_at_root;
    }
  });

  /** Applying over hand-made folders replaces them: that asks for a confirmation first. */
  async function applyStructure(): Promise<void> {
    if (menu?.custom && !confirmApply) {
      confirmApply = true;
      return;
    }
    const structure: Structure =
      kind === "count" ? { kind, max: Math.max(1, Math.floor(max || 1)) } : { kind };
    await libraryStore.setStructure({ structure, originals_at_root: originalsAtRoot });
    confirmApply = false;
    structureOpen = false;
  }

  /* Folder editing */
  let creating = $state(false);
  let newName = $state("");

  function startCreate(): void {
    creating = true;
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
      default:
        return;
    }
    event.preventDefault();
  }

  /** Focuses a field when it appears (inline rename and new-folder boxes). */
  function autofocus(node: HTMLElement): void {
    node.focus();
  }

  function closeStructure(): void {
    structureOpen = false;
    confirmApply = false;
  }

  function onwindowkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      creating = false;
    }
  }
</script>

<svelte:window onkeydown={onwindowkeydown} />

<div class="pane">
  <header>
    <h3 class="caps">Folders</h3>
    <div class="tools">
      {#if libraryStore.menuBusy}
        <span class="spinner" aria-label="Saving the folders" role="status"></span>
      {/if}
      <button type="button" class="icon-btn" aria-label="New folder" title="New folder in the selected folder" onclick={startCreate} disabled={libraryStore.menuBusy}>
        <Icon name="folderPlus" size={18} />
      </button>
      <button type="button" class="icon-btn" aria-label="Automatic folder structure" title="Automatic folder structure" onclick={() => (structureOpen = true)}>
        <Icon name="settings" size={18} />
      </button>
    </div>
  </header>

  {#if menu}
    <ul class="tree" role="tree" aria-label="Console folders" bind:this={tree} {onkeydown}>
      <TreeNode path={[]} name="Console menu" entries={menu.entries} depth={0} {expanded} />
    </ul>
  {:else}
    <p class="muted loading"><span class="spinner" aria-hidden="true"></span> Loading…</p>
  {/if}

  <footer>
    {#if creating}
      <form class="inline" onsubmit={(e) => { e.preventDefault(); void commitCreate(); }}>
        <input type="text" placeholder="Folder name" bind:value={newName} aria-label="New folder name" use:autofocus />
        <button type="submit" class="btn" disabled={libraryStore.menuBusy}>Add</button>
        <button type="button" class="btn btn-quiet" onclick={() => (creating = false)}>Cancel</button>
      </form>
    {:else}
      <p class="muted hint">Drag games or folders onto a folder to move them.</p>
    {/if}
  </footer>
</div>

<Dialog open={structureOpen} title="Folder structure" width="460px" onclose={closeStructure}>
  <p class="muted">How the console groups the games. Applying replaces folders arranged by hand; the games themselves stay.</p>
  <fieldset>
    <legend class="visually-hidden">Structure</legend>
    <label><input type="radio" name="structure" value="flat" bind:group={kind} data-autofocus /> One list</label>
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
  {#if confirmApply}
    <div class="confirm" role="group" aria-label="Confirm replacing the folders">
      <p>This replaces the folders you arranged by hand. The games themselves stay.</p>
    </div>
  {/if}
  {#snippet actions()}
    {#if confirmApply}
      <button type="button" class="btn btn-danger" onclick={applyStructure} disabled={libraryStore.menuBusy}>Replace the folders</button>
      <button type="button" class="btn btn-quiet" onclick={() => (confirmApply = false)}>Keep them</button>
    {:else}
      <button type="button" class="btn btn-primary" onclick={applyStructure} disabled={libraryStore.menuBusy}>
        {#if libraryStore.menuBusy}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
        Apply
      </button>
      <button type="button" class="btn btn-quiet" onclick={closeStructure}>Cancel</button>
    {/if}
  {/snippet}
</Dialog>

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
    padding: var(--space-2) var(--space-2) 0 var(--space-3);
  }

  .tools {
    display: flex;
    gap: var(--space-1);
    align-items: center;
  }

  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: var(--space-2) 0 0;
    padding: 0 var(--space-2);
  }

  .loading {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3);
  }

  footer {
    border-top: 1px solid var(--border);
    padding: var(--space-2) var(--space-3);
    font-size: var(--text-md);
    min-height: 48px;
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

  .hint {
    font-size: var(--text-sm);
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
    width: 64px;
    min-height: var(--control-sm);
    padding: 0 var(--space-1);
  }

</style>
