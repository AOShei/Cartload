<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import { inTauri } from "../../api/tauri";
  import type { Game, GameEdit, MenuEntry, MenuPath } from "../../api/types";
  import { previewDialog } from "../../dev/preview";
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
      compress: g.compress,
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

  /* The remove confirmation lives in the store so Delete in the grid can open it. */
  $effect(() => {
    libraryStore.selected;
    libraryStore.removePrompt = false;
    libraryStore.removeFolderPrompt = false;
  });

  /* Folder rename, from the folder's detail panel. */
  let folderName = $state("");
  $effect(() => {
    const tile = folderTile(single);
    untrack(() => {
      folderName = tile ? tile.folder.name : "";
    });
  });

  async function removeFolder(path: MenuPath): Promise<void> {
    if (await libraryStore.edit({ op: "remove_folder", path })) {
      libraryStore.removeFolderPrompt = false;
    }
  }

  async function renameFolder(path: MenuPath): Promise<void> {
    const name = folderName.trim();
    if (!name) {
      return;
    }
    await libraryStore.edit({ op: "rename_folder", path, name });
  }

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

  /* The description and the advanced fields edit in dialogs (docs/design.md): the panel
     keeps its shape and the editors get the room. Save writes the draft into the form and
     saves the game; Cancel and Escape drop the draft. */
  let descOpen = $state(false);
  let descDraft = $state("");
  let advOpen = $state(false);
  let advDraft = $state({ application: "", args: "", compress: false });

  function openDescription(): void {
    descDraft = form?.description ?? "";
    descOpen = true;
  }

  async function saveDescription(): Promise<void> {
    if (!form) {
      return;
    }
    form.description = descDraft;
    descOpen = false;
    await save();
  }

  function openAdvanced(): void {
    if (form) {
      advDraft = { application: form.application, args: form.args, compress: form.compress ?? false };
      advOpen = true;
    }
  }

  onMount(() => {
    if (!inTauri) {
      const wanted = previewDialog();
      if (wanted === "description") {
        openDescription();
      } else if (wanted === "advanced") {
        openAdvanced();
      }
    }
  });

  /** The flag hakchi's launcher scripts take to run a game through RetroArch. */
  const RETROARCH_FLAG = "--retroarch";

  function usesRetroarch(args: string): boolean {
    return args.split(/\s+/).includes(RETROARCH_FLAG);
  }

  /** The arguments with the RetroArch flag added or removed. */
  function withEmulator(args: string, retroarch: boolean): string {
    const words = args.split(/\s+/).filter((w) => w !== "" && w !== RETROARCH_FLAG);
    if (retroarch) {
      words.push(RETROARCH_FLAG);
    }
    return words.join(" ");
  }

  /** Sets the emulator of the selected games at once (the multi-selection panel). */
  async function applyEmulator(retroarch: boolean): Promise<void> {
    for (const view of selectedGames) {
      const edit = toEdit(view.game);
      const args = withEmulator(edit.args, retroarch);
      if (args !== edit.args) {
        await libraryStore.save({ ...edit, args });
      }
    }
  }

  async function saveAdvanced(): Promise<void> {
    if (!form) {
      return;
    }
    form.application = advDraft.application;
    form.args = advDraft.args;
    form.compress = advDraft.compress;
    advOpen = false;
    await save();
  }

  async function removeSelected(): Promise<void> {
    const codes = selectedGames.map((g) => g.game.code);
    await libraryStore.remove(codes);
    libraryStore.removePrompt = false;
  }

  /** Every folder of the tree, for "Move to". */
  const folders = $derived.by((): { path: MenuPath; label: string }[] => {
    const out: { path: MenuPath; label: string }[] = [{ path: [], label: "Console menu" }];
    const walk = (entries: MenuEntry[], prefix: MenuPath, depth: number) => {
      entries.forEach((entry, index) => {
        if (entry.kind === "folder") {
          const path = [...prefix, index];
          out.push({ path, label: `${"  ".repeat(depth)}${entry.name}` });
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
        <img src={game.cover} alt="Cover of {game.game.name}" decoding="async" crossorigin="anonymous" />
        {#if game.game.placeholder_art}
          <span class="noart">No art yet</span>
        {/if}
      </div>
      <div class="art-actions">
        <button type="button" class="btn" onclick={() => libraryStore.fetchArt([game.game.code], true)} disabled={libraryStore.artBusy}>
          {#if libraryStore.artBusy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="download" size={16} />{/if}
          <span>{libraryStore.artBusy ? "Fetching…" : "Fetch art"}</span>
        </button>
        <button type="button" class="btn btn-quiet" onclick={() => libraryStore.chooseCover(game.game.code)} disabled={libraryStore.coverBusy}>
          {#if libraryStore.coverBusy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="image" size={16} />{/if}
          <span>{libraryStore.coverBusy ? "Replacing…" : "Choose image…"}</span>
        </button>
      </div>
      {#if artLine}
        <p class="art-line muted" role="status">{artLine}</p>
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
          <label class="players">
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
          <span>Emulator</span>
          <select value={usesRetroarch(form.args) ? "retroarch" : "builtin"} onchange={(e) => { if (form) form.args = withEmulator(form.args, e.currentTarget.value === "retroarch"); }}>
            <option value="builtin">{game.system_name === "NES" ? "The console's own NES emulator" : "The console's own SNES emulator"}</option>
            <option value="retroarch">RetroArch ({game.system_name === "NES" ? "FCEUmm core" : "Snes9x core"})</option>
          </select>
        </label>
        <div class="field">
          <span class="field-label" id="description-label">Description</span>
          <button type="button" class="desc-preview" onclick={openDescription} aria-labelledby="description-label" aria-describedby="description-text" title="Edit the description">
            <span class="clamp" id="description-text" class:muted={form.description.trim() === ""}>
              {form.description.trim() === "" ? "No description yet." : form.description}
            </span>
            <span class="edit-tag" aria-hidden="true">Edit</span>
          </button>
        </div>
        {#if dirty}
          <div class="save-row">
            <button type="submit" class="btn btn-primary" disabled={libraryStore.saving}>
              {#if libraryStore.saving}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
              {libraryStore.saving ? "Saving…" : "Save"}
            </button>
            <button type="button" class="btn btn-quiet" onclick={revert} disabled={libraryStore.saving}>Revert</button>
          </div>
        {/if}

        <div class="advanced-row">
          <button type="button" class="btn" onclick={openAdvanced}>
            <Icon name="settings" size={16} />
            <span>Advanced…</span>
          </button>
        </div>
      </form>

      <Dialog open={descOpen} title="Description of {game.game.name}" width="640px" onclose={() => (descOpen = false)}>
        <textarea class="editor" rows="14" bind:value={descDraft} aria-label="Description" data-autofocus></textarea>
        <p class="muted small">Shown on the console with the game's cover.</p>
        {#snippet actions()}
          <button type="button" class="btn btn-primary" onclick={saveDescription} disabled={libraryStore.saving}>
            {#if libraryStore.saving}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
            {libraryStore.saving ? "Saving…" : "Save"}
          </button>
          <button type="button" class="btn btn-quiet" onclick={() => (descOpen = false)}>Cancel</button>
        {/snippet}
      </Dialog>

      <Dialog open={advOpen} title="Advanced: {game.game.name}" width="600px" onclose={() => (advOpen = false)}>
        <label>
          <span>Launch command</span>
          <input type="text" class="mono" bind:value={advDraft.application} data-autofocus />
        </label>
        <label>
          <span>Arguments</span>
          <input type="text" class="mono" bind:value={advDraft.args} />
        </label>
        <p class="muted small">The Emulator choice in the game's details sets or clears <span class="mono">--retroarch</span> here; anything else is passed to the emulator as typed.</p>
        <label class="check">
          <input type="checkbox" bind:checked={advDraft.compress} />
          <span>Compress for the console (7z, needs the tiny7zx mod; roughly halves the space a SNES game takes)</span>
        </label>
        <p class="mono preview">{advDraft.application} /var/games/{game.game.code}/{game.game.rom_file}{advDraft.compress ? ".7z" : ""} {advDraft.args}</p>
        <dl class="facts">
          <dt>Code</dt><dd class="mono">{game.game.code}</dd>
          <dt>ROM</dt><dd class="mono">{game.game.rom_file}</dd>
          <dt>Imported from</dt><dd>{game.game.original_file}</dd>
          <dt>CRC-32</dt><dd class="mono">{game.game.crc32.toString(16).toUpperCase().padStart(8, "0")}</dd>
          <dt>Size on the console</dt><dd class="num">{formatBytes(game.size)}{game.game.compress ? " (compressed)" : ""}</dd>
          <dt>Saves</dt><dd class="num">{game.game.save_count}</dd>
          {#if game.game.db_name}<dt>Hash database</dt><dd>{game.game.db_name}</dd>{/if}
          {#if game.game.tgdb_id}<dt>TheGamesDB</dt><dd class="num">{game.game.tgdb_id}</dd>{/if}
        </dl>
        {#snippet actions()}
          <button type="button" class="btn btn-primary" onclick={saveAdvanced} disabled={libraryStore.saving}>
            {#if libraryStore.saving}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
            {libraryStore.saving ? "Saving…" : "Save"}
          </button>
          <button type="button" class="btn btn-quiet" onclick={() => (advOpen = false)}>Cancel</button>
        {/snippet}
      </Dialog>

    </div>
  {:else if single?.kind === "original"}
    <div class="scroll">
      {#if single.cover}
        <div class="cover">
          <img src={single.cover} alt="Cover of {single.name}" decoding="async" crossorigin="anonymous" />
        </div>
      {:else}
        <div class="cover plain"><Icon name="builtIn" size={48} /></div>
      {/if}
      <h3>{single.name}</h3>
      <p class="muted">Built into the console. It can be moved between folders but not changed or removed.</p>
      <p class="mono muted">{single.code}</p>
    </div>
  {:else if folderTile(single)}
    {@const tile = folderTile(single)!}
    <div class="scroll">
      {#if tile.cover}
        <div class="cover"><img src={tile.cover} alt="Picture of the folder {tile.folder.name}" decoding="async" crossorigin="anonymous" /></div>
      {:else}
        <div class="cover plain folder"><Icon name="folder" size={48} /></div>
      {/if}
      <div class="art-actions">
        <button type="button" class="btn" onclick={() => void libraryStore.chooseFolderImage(tile.path)} disabled={libraryStore.menuBusy}>
          <Icon name="image" size={16} />
          <span>{tile.cover ? "Change picture…" : "Choose picture…"}</span>
        </button>
        {#if tile.cover}
          <button type="button" class="btn btn-quiet" onclick={() => void libraryStore.clearFolderImage(tile.path)} disabled={libraryStore.menuBusy}>
            Plain icon
          </button>
        {/if}
      </div>
      <form class="fields" onsubmit={(e) => { e.preventDefault(); void renameFolder(tile.path); }}>
        <label>
          <span>Name</span>
          <input type="text" bind:value={folderName} required />
        </label>
        {#if folderName.trim() !== "" && folderName !== tile.folder.name}
          <div class="save-row">
            <button type="submit" class="btn btn-primary" disabled={libraryStore.menuBusy}>
              {#if libraryStore.menuBusy}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
              Rename
            </button>
            <button type="button" class="btn btn-quiet" onclick={() => (folderName = tile.folder.name)} disabled={libraryStore.menuBusy}>Revert</button>
          </div>
        {/if}
      </form>
      <p class="muted">{tile.games} game{tile.games === 1 ? "" : "s"} inside. The picture shows on the console's menu, like a game's cover. Double-click the folder, or press Enter, to open it.</p>
      <div class="art-actions">
        <button type="button" class="btn" onclick={() => libraryStore.openFolder(tile.path)}>Open folder</button>
        <button type="button" class="btn btn-quiet" onclick={() => (libraryStore.removeFolderPrompt = true)} title="Remove folder (Delete)">
          <Icon name="trash" size={16} />
          <span>Remove…</span>
        </button>
      </div>
      <Dialog open={libraryStore.removeFolderPrompt} title="Remove this folder?" width="460px" onclose={() => (libraryStore.removeFolderPrompt = false)}>
        <p>“{tile.folder.name}” goes away; its {tile.games === 1 ? "game moves" : `${tile.games} games move`} up one level. Nothing leaves the library.</p>
        {#snippet actions()}
          <button type="button" class="btn btn-danger" onclick={() => void removeFolder(tile.path)} disabled={libraryStore.menuBusy}>
            {#if libraryStore.menuBusy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="trash" size={16} />{/if}
            Remove folder
          </button>
          <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (libraryStore.removeFolderPrompt = false)}>Keep</button>
        {/snippet}
      </Dialog>
    </div>
  {:else if count > 1}
    <div class="scroll">
      <h3>{count} selected</h3>
      <p class="muted">{selectedGames.length} game{selectedGames.length === 1 ? "" : "s"} from the library.</p>
      <div class="bulk">
        <button type="button" class="btn" onclick={() => libraryStore.fetchArt(selectedGames.map((g) => g.game.code), false)} disabled={libraryStore.artBusy || selectedGames.length === 0}>
          {#if libraryStore.artBusy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="download" size={16} />{/if}
          <span>{libraryStore.artBusy ? "Fetching…" : `Fetch art for ${selectedGames.length} game${selectedGames.length === 1 ? "" : "s"}`}</span>
        </button>
        {#if artLine}<p class="art-line muted" role="status">{artLine}</p>{/if}
        {#if selectedGames.length > 0}
          <label class="move">
            <span>Emulator</span>
            <select onchange={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ""; if (v) void applyEmulator(v === "retroarch"); }} disabled={libraryStore.saving}>
              <option value="">Choose for {selectedGames.length} games…</option>
              <option value="builtin">The console's own emulator</option>
              <option value="retroarch">RetroArch</option>
            </select>
          </label>
        {/if}
        <label class="move">
          <span>Move to</span>
          <select onchange={(e) => { moveTo(e.currentTarget.value); e.currentTarget.value = ""; }} disabled={libraryStore.menuBusy}>
            <option value="">Choose a folder…</option>
            {#each folders as f (f.path.join("/"))}
              <option value={f.path.length === 0 ? "root" : f.path.join("/")}>{f.label}</option>
            {/each}
          </select>
        </label>
        {#if selectedGames.length > 0}
          <button type="button" class="btn btn-quiet" onclick={() => (libraryStore.removePrompt = true)} title="Remove (Delete)">
            <Icon name="trash" size={16} />
            <span>Remove {selectedGames.length} games…</span>
          </button>
        {/if}
      </div>
    </div>
  {:else}
    <div class="scroll empty-note">
      <p>Select a game to see and edit its details.</p>
      <p class="small">Ctrl-click or Shift-click to select several. Drag games onto a folder to move them.</p>
    </div>
  {/if}
</div>

<Dialog
  open={libraryStore.removePrompt && selectedGames.length > 0}
  title={selectedGames.length === 1 ? "Remove this game?" : `Remove ${selectedGames.length} games?`}
  width="460px"
  onclose={() => (libraryStore.removePrompt = false)}
>
  {#if selectedGames.length === 1}
    <p>“{selectedGames[0]?.game.name}” leaves the library now and the console at the next sync. The ROM file you imported is not touched.</p>
  {:else}
    <p>They leave the library now and the console at the next sync. The ROM files you imported are not touched.</p>
  {/if}
  {#snippet actions()}
    <button type="button" class="btn btn-danger" onclick={removeSelected} disabled={libraryStore.removing}>
      {#if libraryStore.removing}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="trash" size={16} />{/if}
      {libraryStore.removing ? "Removing…" : selectedGames.length === 1 ? "Remove game" : `Remove ${selectedGames.length} games`}
    </button>
    <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (libraryStore.removePrompt = false)}>Keep</button>
  {/snippet}
</Dialog>

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
    font-size: var(--text-md);
  }

  .scroll.empty-note {
    justify-content: center;
  }

  .small {
    font-size: var(--text-sm);
  }

  .cover {
    position: relative;
    flex: none;
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
    bottom: var(--space-2);
    right: var(--space-2);
    padding: 1px var(--space-2);
    border-radius: 999px;
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    color: var(--text-muted);
    font-size: var(--text-xs);
  }

  .art-actions,
  .save-row {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .art-line {
    font-size: var(--text-sm);
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
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  input[type="text"],
  textarea,
  select {
    width: 100%;
  }

  textarea {
    resize: vertical;
  }

  .row {
    display: flex;
    gap: var(--space-3);
    align-items: flex-end;
  }

  /* "Players [1]  [ ] At the same time" on one line. */
  .row {
    align-items: center;
    min-height: var(--control);
  }

  .players {
    flex-direction: row;
    align-items: center;
    gap: var(--space-2);
  }

  .players select {
    width: 64px;
  }

  .check {
    flex-direction: row;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control);
  }

  .check > span {
    font-size: var(--text-md);
    color: var(--text);
  }

  .check.off > span {
    color: var(--text-faint);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .field-label {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  /* Three lines of the description, read-only; the whole block opens the editor. */
  .desc-preview {
    position: relative;
    display: block;
    width: 100%;
    height: calc(3 * var(--text-md) * var(--leading) + 2 * var(--space-2) + 2px);
    padding: var(--space-2) var(--space-2);
    padding-right: 52px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--surface);
    color: var(--text);
    text-align: left;
    cursor: text;
    overflow: hidden;
  }

  .desc-preview:hover {
    background: var(--surface-raised);
  }

  .clamp {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: var(--leading);
  }

  .edit-tag {
    position: absolute;
    top: var(--space-1);
    right: var(--space-1);
    padding: 1px var(--space-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--surface);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }

  .desc-preview:hover .edit-tag {
    color: var(--text);
  }

  .advanced-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .editor {
    width: 100%;
    min-height: 240px;
    resize: vertical;
    font-size: var(--text-base);
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
    font-size: var(--text-sm);
  }

  .facts dt {
    color: var(--text-muted);
  }

  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
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

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }
</style>
