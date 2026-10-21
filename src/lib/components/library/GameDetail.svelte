<script lang="ts">
  import { untrack } from "svelte";
  import Icon from "../Icon.svelte";
  import type { Game, GameEdit, MenuEntry, MenuPath } from "../../api/types";
  import { formatBytes, libraryStore, type Tile } from "../../state";

  const single = $derived(libraryStore.single);
  const game = $derived(single?.kind === "game" ? single.view : null);
  const selectedGames = $derived(libraryStore.selectedGames);
  const count = $derived(libraryStore.selected.length);

  function toEdit(g: Game): GameEdit {
    return {
      code: g.code,
      name: g.name,
      sort_name: g.sort_name,
      publisher: g.publisher === "UNKNOWN" ? "" : g.publisher,
      release_date: g.release_date === "1900-01-01" ? "" : g.release_date,
      players: g.players,
      simultaneous: g.simultaneous,
      copyright: g.copyright,
      description: g.description,
      application: g.application,
      args: g.args,
    };
  }

  const snapshot = $derived(game ? toEdit(game.game) : null);
  let form = $state<GameEdit | null>(null);
  const dirty = $derived(form !== null && snapshot !== null && JSON.stringify(form) !== JSON.stringify(snapshot));

  $effect(() => {
    const next = snapshot;
    untrack(() => {
      if (!form || !next || form.code !== next.code || !dirty) {
        form = next ? { ...next } : null;
      }
    });
  });

  let confirmRemove = $state(false);
  $effect(() => {
    libraryStore.selected;
    confirmRemove = false;
  });

  async function save(): Promise<void> {
    if (!form || !dirty) {
      return;
    }
    const saved = await libraryStore.save(form);
    if (saved) {
      form = toEdit(saved.game);
    }
  }

  function revert(): void {
    form = snapshot ? { ...snapshot } : null;
  }

  async function removeSelected(): Promise<void> {
    const codes = selectedGames.map((g) => g.game.code);
    confirmRemove = false;
    await libraryStore.remove(codes);
  }

  /** Every folder of the tree, for "Move to". */
  const folders = $derived.by((): { path: MenuPath; label: string }[] => {
    const out: { path: MenuPath; label: string }[] = [{ path: [], label: "Console menu" }];
    const walk = (entries: MenuEntry[], prefix: MenuPath, depth: number) => {
      entries.forEach((entry, index) => {
        if (entry.kind === "folder") {
          const path = [...prefix, index];
          out.push({ path, label: `${"  ".repeat(depth)}${entry.name}` });
          walk(entry.entries, path, depth + 1);
        }
      });
    };
    walk(libraryStore.menu?.entries ?? [], [], 1);
    return out;
  });

  function moveTo(value: string): void {
    if (value === "") {
      return;
    }
    const path = value === "root" ? [] : value.split("/").map(Number);
    void libraryStore.move(libraryStore.itemsFor(libraryStore.selected), path);
  }

  const artLine = $derived.by((): string | null => {
    const p = libraryStore.artProgress;
    if (!libraryStore.artBusy) {
      return null;
    }
    if (!p) {
      return "Contacting TheGamesDB…";
    }
    const head = p.total > 1 ? `${p.done} of ${p.total} · ` : "";
    switch (p.event.event) {
      case "searching":
        return `${head}Searching for “${p.event.query}”…`;
      case "matched":
        return `${head}Found “${p.event.title}”.`;
      case "no_match":
        return `${head}Nothing found.`;
      case "fetching":
        return `${head}Fetching ${p.event.what}…`;
      case "done":
        return `${head}Done.`;
      case "failed":
        return `${head}Failed: ${p.event.error}`;
      default:
        return head;
    }
  });

  function folderTile(tile: Tile | null) {
    return tile?.kind === "folder" ? tile : null;
  }
</script>

<div class="pane">
  {#if game && form}
    <div class="scroll">
      <div class="cover">
        <img src={game.cover} alt="Cover of {game.game.name}" />
        {#if game.game.placeholder_art}
          <span class="noart">No art yet</span>
        {/if}
      </div>
      <div class="art-actions">
        <button type="button" class="btn" onclick={() => libraryStore.fetchArt([game.game.code], true)} disabled={libraryStore.artBusy}>
          <Icon name="download" size={16} />
          <span>Fetch art</span>
        </button>
        <button type="button" class="btn btn-quiet" onclick={() => libraryStore.chooseCover(game.game.code)}>
          <Icon name="image" size={16} />
          <span>Choose image…</span>
        </button>
      </div>
      {#if artLine}
        <p class="art-line muted">{artLine}</p>
      {/if}

      <form class="fields" onsubmit={(e) => { e.preventDefault(); void save(); }}>
        <label>
          <span>Name</span>
          <input type="text" bind:value={form.name} required />
        </label>
        <label>
          <span>Sort name</span>
          <input type="text" bind:value={form.sort_name} placeholder={game.game.name.toLowerCase()} />
        </label>
        <label>
          <span>Publisher</span>
          <input type="text" bind:value={form.publisher} placeholder="Unknown" />
        </label>
        <label>
          <span>Release date</span>
          <input type="text" class="num" bind:value={form.release_date} placeholder="YYYY-MM-DD" pattern="\d{'{'}4{'}'}-\d{'{'}2{'}'}-\d{'{'}2{'}'}" />
        </label>
        <div class="row">
          <label>
            <span>Players</span>
            <select bind:value={form.players}>
              <option value={1}>1</option>
              <option value={2}>2</option>
            </select>
          </label>
          <label class="check" class:off={form.players !== 2}>
            <input type="checkbox" bind:checked={form.simultaneous} disabled={form.players !== 2} />
            <span>At the same time</span>
          </label>
        </div>
        <label>
          <span>Description</span>
          <textarea rows="3" bind:value={form.description}></textarea>
        </label>
        {#if dirty}
          <div class="save-row">
            <button type="submit" class="btn" disabled={libraryStore.saving}>{libraryStore.saving ? "Saving…" : "Save"}</button>
            <button type="button" class="btn btn-quiet" onclick={revert}>Revert</button>
          </div>
        {/if}

        <details class="advanced">
          <summary>Advanced</summary>
          <label>
            <span>Launch command</span>
            <input type="text" class="mono" bind:value={form.application} />
          </label>
          <label>
            <span>Arguments</span>
            <input type="text" class="mono" bind:value={form.args} />
          </label>
          <p class="mono preview">{form.application} /var/games/{game.game.code}/{game.game.rom_file} {form.args}</p>
          <dl class="facts">
            <dt>Code</dt><dd class="mono">{game.game.code}</dd>
            <dt>ROM</dt><dd class="mono">{game.game.rom_file}</dd>
            <dt>Imported from</dt><dd>{game.game.original_file}</dd>
            <dt>CRC-32</dt><dd class="mono">{game.game.crc32.toString(16).toUpperCase().padStart(8, "0")}</dd>
            <dt>Size</dt><dd class="num">{formatBytes(game.size)}</dd>
            <dt>Saves</dt><dd class="num">{game.game.save_count}</dd>
            {#if game.game.db_name}<dt>Hash database</dt><dd>{game.game.db_name}</dd>{/if}
            {#if game.game.tgdb_id}<dt>TheGamesDB</dt><dd class="num">{game.game.tgdb_id}</dd>{/if}
          </dl>
        </details>
      </form>

      <div class="remove">
        {#if confirmRemove}
          <p>Remove “{game.game.name}” from the library? It leaves the console at the next sync.</p>
          <div class="save-row">
            <button type="button" class="btn danger" onclick={removeSelected} disabled={libraryStore.removing}>Remove 1 game</button>
            <button type="button" class="btn btn-quiet" onclick={() => (confirmRemove = false)}>Keep</button>
          </div>
        {:else}
          <button type="button" class="btn btn-quiet" onclick={() => (confirmRemove = true)}>
            <Icon name="trash" size={16} />
            <span>Remove from the library…</span>
          </button>
        {/if}
      </div>
    </div>
  {:else if single?.kind === "original"}
    <div class="scroll">
      <div class="cover plain"><Icon name="builtIn" size={48} /></div>
      <h3>{single.name}</h3>
      <p class="muted">Built into the console. It can be moved between folders but not changed or removed.</p>
      <p class="mono muted">{single.code}</p>
    </div>
  {:else if folderTile(single)}
    {@const tile = folderTile(single)!}
    <div class="scroll">
      <div class="cover plain folder"><Icon name="folder" size={48} /></div>
      <h3>{tile.folder.name}</h3>
      <p class="muted">{tile.games} game{tile.games === 1 ? "" : "s"} inside. Double-click it, or press Enter, to open it. Rename or remove it from the folder tree.</p>
      <button type="button" class="btn" onclick={() => libraryStore.openFolder(tile.path)}>Open folder</button>
    </div>
  {:else if count > 1}
    <div class="scroll">
      <h3>{count} selected</h3>
      <p class="muted">{selectedGames.length} game{selectedGames.length === 1 ? "" : "s"} from the library.</p>
      <div class="bulk">
        <button type="button" class="btn" onclick={() => libraryStore.fetchArt(selectedGames.map((g) => g.game.code), false)} disabled={libraryStore.artBusy || selectedGames.length === 0}>
          <Icon name="download" size={16} />
          <span>Fetch art for {selectedGames.length} game{selectedGames.length === 1 ? "" : "s"}</span>
        </button>
        {#if artLine}<p class="art-line muted">{artLine}</p>{/if}
        <label class="move">
          <span>Move to</span>
          <select onchange={(e) => { moveTo(e.currentTarget.value); e.currentTarget.value = ""; }}>
            <option value="">Choose a folder…</option>
            {#each folders as f (f.path.join("/"))}
              <option value={f.path.length === 0 ? "root" : f.path.join("/")}>{f.label}</option>
            {/each}
          </select>
        </label>
        {#if selectedGames.length > 0}
          {#if confirmRemove}
            <p>Remove {selectedGames.length} games from the library? They leave the console at the next sync.</p>
            <div class="save-row">
              <button type="button" class="btn danger" onclick={removeSelected} disabled={libraryStore.removing}>Remove {selectedGames.length} games</button>
              <button type="button" class="btn btn-quiet" onclick={() => (confirmRemove = false)}>Keep</button>
            </div>
          {:else}
            <button type="button" class="btn btn-quiet" onclick={() => (confirmRemove = true)}>
              <Icon name="trash" size={16} />
              <span>Remove {selectedGames.length} games…</span>
            </button>
          {/if}
        {/if}
      </div>
    </div>
  {:else}
    <div class="scroll empty">
      <p class="muted">Select a game to see and edit its details.</p>
      <p class="muted small">Ctrl-click or Shift-click to select several. Drag games onto a folder to move them.</p>
    </div>
  {/if}
</div>

<style>
  .pane {
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-3) var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    font-size: 0.9rem;
  }

  .empty {
    justify-content: center;
    text-align: center;
  }

  .small {
    font-size: 0.8rem;
  }

  h3 {
    margin: 0;
    font-size: 1.05rem;
  }

  .cover {
    position: relative;
    aspect-ratio: 228 / 204;
    border-radius: var(--radius-m);
    overflow: hidden;
    background: var(--surface-raised);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-faint);
  }

  .cover.folder {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .noart {
    position: absolute;
    bottom: 6px;
    right: 6px;
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    color: var(--text-muted);
    font-size: 0.7rem;
  }

  .art-actions,
  .save-row {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .art-line {
    font-size: 0.8rem;
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  label > span {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  input[type="text"],
  textarea,
  select {
    width: 100%;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--surface);
    font: inherit;
    color: inherit;
  }

  textarea {
    resize: vertical;
  }

  .row {
    display: flex;
    gap: var(--space-3);
    align-items: flex-end;
  }

  .row label:first-child {
    width: 5rem;
  }

  .check {
    flex-direction: row;
    align-items: center;
    gap: var(--space-2);
    padding-bottom: 0.4rem;
  }

  .check > span {
    font-size: 0.85rem;
    color: var(--text);
  }

  .check.off > span {
    color: var(--text-faint);
  }

  .advanced {
    margin-top: var(--space-2);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .advanced summary {
    font-size: 0.85rem;
  }

  .preview {
    word-break: break-all;
    color: var(--text-muted);
    user-select: text;
  }

  .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 2px var(--space-3);
    margin: 0;
    font-size: 0.8rem;
  }

  .facts dt {
    color: var(--text-muted);
  }

  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .remove {
    margin-top: auto;
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
  }

  .bulk {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: flex-start;
  }

  .move {
    width: 100%;
  }
</style>
