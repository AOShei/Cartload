<script lang="ts">
  import type { SvelteSet } from "svelte/reactivity";
  import TreeNode from "./TreeNode.svelte";
  import Icon from "../Icon.svelte";
  import type { MenuEntry, MenuFolder, MenuPath } from "../../api/types";
  import { DRAG_TYPE, countGames, libraryStore, pathKey, samePath } from "../../state";

  interface Props {
    path: MenuPath;
    name: string;
    entries: MenuEntry[];
    depth: number;
    expanded: SvelteSet<string>;
  }

  let { path, name, entries, depth, expanded }: Props = $props();

  const key = $derived(pathKey(path));
  const isRoot = $derived(path.length === 0);
  const children = $derived(
    entries.flatMap((entry, index): { folder: MenuFolder; index: number }[] =>
      entry.kind === "folder" ? [{ folder: entry, index }] : [],
    ),
  );
  const open = $derived(isRoot || expanded.has(key));
  const selected = $derived(samePath(libraryStore.folder, path));
  const games = $derived(countGames(entries));
  let dropping = $state(false);

  function toggle(event: Event): void {
    event.stopPropagation();
    if (isRoot) {
      return;
    }
    if (expanded.has(key)) {
      expanded.delete(key);
    } else {
      expanded.add(key);
    }
  }

  function ondragstart(event: DragEvent): void {
    if (isRoot || !event.dataTransfer) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.setData(DRAG_TYPE, JSON.stringify([{ kind: "folder", path }]));
    event.dataTransfer.effectAllowed = "move";
  }

  function ondragover(event: DragEvent): void {
    if (event.dataTransfer?.types.includes(DRAG_TYPE)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      dropping = true;
    }
  }

  function ondrop(event: DragEvent): void {
    dropping = false;
    const raw = event.dataTransfer?.getData(DRAG_TYPE);
    if (!raw) {
      return;
    }
    event.preventDefault();
    void libraryStore.move(JSON.parse(raw), path);
    expanded.add(key);
  }
</script>

<li role="treeitem" aria-expanded={children.length > 0 ? open : undefined} aria-selected={selected} aria-level={depth + 1}>
  <div
    class="row"
    role="presentation"
    class:selected
    class:dropping
    style:padding-left="{depth * 14 + 4}px"
    draggable={!isRoot}
    data-tree-row
    data-path={key}
    {ondragstart}
    {ondragover}
    ondragleave={() => (dropping = false)}
    {ondrop}
  >
    {#if children.length > 0 && !isRoot}
      <button
        type="button"
        class="twisty"
        class:open
        aria-label={open ? `Collapse ${name}` : `Expand ${name}`}
        tabindex="-1"
        onclick={toggle}
      >
        <Icon name="chevron" size={14} />
      </button>
    {:else}
      <span class="twisty"></span>
    {/if}
    <!-- One tab stop for the tree: the folder being viewed. Arrows move between rows. -->
    <button
      type="button"
      class="label focus-inset"
      tabindex={selected ? 0 : -1}
      onclick={() => libraryStore.openFolder(path)}
      ondblclick={toggle}
    >
      <span class="glyph"><Icon name={isRoot ? "console" : "folder"} size={16} /></span>
      <span class="name">{name}</span>
      <span class="count num">{games}</span>
    </button>
  </div>
  {#if children.length > 0 && open}
    <ul role="group">
      {#each children as child (child.index)}
        <TreeNode
          path={[...path, child.index]}
          name={child.folder.name}
          entries={child.folder.entries}
          depth={depth + 1}
          {expanded}
        />
      {/each}
    </ul>
  {/if}
</li>

<style>
  li {
    list-style: none;
  }

  ul {
    margin: 0;
    padding: 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-s);
    border: 1px solid transparent;
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  .row:hover {
    background: var(--surface-hover);
  }

  .row.selected {
    background: var(--accent-soft);
  }

  .row.selected .label {
    color: var(--accent-strong);
    font-weight: var(--weight-semibold);
  }

  .row.dropping {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .twisty {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    flex: none;
  }

  .twisty :global(svg) {
    transition: transform var(--motion-fast) var(--ease-out);
  }

  .twisty.open :global(svg) {
    transform: rotate(90deg);
  }

  .label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
    min-height: 28px;
    padding: 0 var(--space-1);
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    font-size: var(--text-md);
  }

  .glyph {
    display: inline-flex;
    color: var(--text-muted);
    flex: none;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }
</style>
