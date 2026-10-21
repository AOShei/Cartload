<script lang="ts">
  import { onMount, tick } from "svelte";
  import { tgdbKeyGet, tgdbKeySet } from "../api/commands";
  import { AppError, inTauri } from "../api/tauri";
  import type { KeyStatus } from "../api/types";
  import AppMark from "../components/AppMark.svelte";
  import Icon from "../components/Icon.svelte";
  import Wordmark from "../components/Wordmark.svelte";
  import { app, libraryStore, navigation, preferences, toasts, type Theme } from "../state";

  let keyStatus = $state<KeyStatus | null>(null);
  let keyLoading = $state(false);
  let keyValue = $state("");
  let keyBusy = $state(false);
  let confirmKeyRemoval = $state(false);
  let about: HTMLElement | undefined = $state();

  onMount(() => {
    void libraryStore.start();
    if (inTauri) {
      keyLoading = true;
      void tgdbKeyGet()
        .then((status) => (keyStatus = status))
        .catch((error: unknown) => toasts.error(`Could not read the key status: ${AppError.from(error).message}`))
        .finally(() => (keyLoading = false));
    }
  });

  /** The sidebar brand opens the About panel. */
  $effect(() => {
    if (navigation.section === "about" && about) {
      navigation.tookSection();
      void tick().then(() => {
        about?.scrollIntoView({ block: "start" });
        about?.focus({ preventScroll: true });
      });
    }
  });

  async function saveKey(clear: boolean): Promise<void> {
    if (!inTauri) {
      toasts.info("Keys are stored inside the desktop app only.");
      return;
    }
    keyBusy = true;
    confirmKeyRemoval = false;
    try {
      keyStatus = await tgdbKeySet(clear ? null : keyValue);
      keyValue = "";
      toasts.success(clear ? "TheGamesDB key removed." : "TheGamesDB key stored.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      keyBusy = false;
    }
  }

  const themes: { id: Theme; label: string; hint: string }[] = [
    { id: "system", label: "Follow the system", hint: "Light or dark, as the desktop is set." },
    { id: "light", label: "Light", hint: "" },
    { id: "dark", label: "Dark", hint: "" },
  ];

  const credits: { name: string; by: string; url: string; what: string }[] = [
    { name: "hakchi", by: "madmonkey", url: "github.com/madmonkey1907/hakchi", what: "the kernel and the device-side tools Cartload installs and talks to (GPL-3)." },
    { name: "hakchi2 CE", by: "TeamShinkansen", url: "github.com/TeamShinkansen/Hakchi2-CE", what: "the reference for the console protocol, the FEL sequence and the sync (GPL-3)." },
    { name: "TheGamesDB", by: "the TheGamesDB community", url: "thegamesdb.net", what: "cover art and game metadata, fetched with your own API key." },
  ];
</script>

<div class="page">
  <section class="card" aria-labelledby="theme-heading">
    <h2 id="theme-heading">Appearance</h2>
    <fieldset>
      <legend class="visually-hidden">Theme</legend>
      {#each themes as theme (theme.id)}
        <label class="option">
          <input
            type="radio"
            name="theme"
            value={theme.id}
            checked={preferences.theme === theme.id}
            onchange={() => preferences.setTheme(theme.id)}
          />
          <span class="option-icon">
            <Icon name={theme.id === "dark" ? "moon" : "sun"} size={16} />
          </span>
          <span>
            {theme.label}
            {#if theme.hint}<span class="muted"> · {theme.hint}</span>{/if}
          </span>
        </label>
      {/each}
    </fieldset>
  </section>

  <section class="card" aria-labelledby="log-heading">
    <h2 id="log-heading">Activity log</h2>
    <label class="option">
      <input
        type="checkbox"
        checked={preferences.showDebugLog}
        onchange={(e) => preferences.setShowDebugLog(e.currentTarget.checked)}
      />
      <span>
        Show details
        <span class="muted"> · every command sent to the console and USB events.</span>
      </span>
    </label>
  </section>

  <section class="card" aria-labelledby="library-heading">
    <h2 id="library-heading">Library</h2>
    <p class="muted">
      Games are kept in <span class="mono path">{libraryStore.view?.root ?? "the user data directory"}</span>.
    </p>
    <h3>TheGamesDB key</h3>
    <p class="muted">
      Cover art and metadata come from TheGamesDB, which needs a free API key from thegamesdb.net.
      {#if keyLoading}
        Checking the stored key…
      {:else if keyStatus?.environment}
        The CARTLOAD_TGDB_KEY variable is set and is used.
      {:else if keyStatus?.stored}
        A key is stored.
      {:else}
        No key yet.
      {/if}
      {#if keyStatus?.remaining !== null && keyStatus?.remaining !== undefined}
        <span class="num">{keyStatus.remaining}</span> requests left this month.
      {/if}
    </p>
    <form class="key" onsubmit={(e) => { e.preventDefault(); void saveKey(false); }}>
      <input
        type="password"
        bind:value={keyValue}
        placeholder={keyStatus?.stored ? "Replace the stored key" : "Paste the key"}
        aria-label="TheGamesDB API key"
        autocomplete="off"
        disabled={keyBusy}
      />
      <button type="submit" class="btn" disabled={keyBusy || !keyValue.trim()}>
        {#if keyBusy}<span class="spinner" aria-hidden="true"></span>{/if}
        Store
      </button>
      {#if keyStatus?.stored && !confirmKeyRemoval}
        <button type="button" class="btn btn-quiet" disabled={keyBusy} onclick={() => (confirmKeyRemoval = true)}>
          Remove…
        </button>
      {/if}
    </form>
    {#if confirmKeyRemoval}
      <div class="confirm" role="group" aria-label="Confirm removing the key">
        <p>Remove the stored key? Fetching art stops working until a key is stored again.</p>
        <div class="confirm-actions">
          <button type="button" class="btn btn-danger" disabled={keyBusy} onclick={() => saveKey(true)}>Remove key</button>
          <button type="button" class="btn btn-quiet" onclick={() => (confirmKeyRemoval = false)}>Keep it</button>
        </div>
      </div>
    {/if}
  </section>

  <section class="card about" aria-labelledby="about-heading" bind:this={about} tabindex="-1">
    <div class="identity">
      <AppMark size={64} />
      <div class="identity-text">
        <h2 id="about-heading" class="visually-hidden">About Cartload</h2>
        <span class="wordmark"><Wordmark height={22} label="Cartload" /></span>
        <p class="muted">
          {#if app.version}
            Version <span class="num">{app.version.app}</span>{app.version.debug ? ", debug build" : ""}
          {:else if app.versionError}
            Version unknown ({app.versionError})
          {:else}
            Reading the version…
          {/if}
        </p>
      </div>
    </div>
    <p>
      Installs the hakchi kernel on NES, SNES and Mega Drive Classic Mini consoles and keeps a
      games library in sync with them.
    </p>
    <p class="muted">
      Free software under the GNU General Public License, version 3 or later. Source at
      <span class="mono">github.com/AOShei/Cartload</span>.
    </p>
    <h3>Built on</h3>
    <ul class="credits">
      {#each credits as credit (credit.name)}
        <li>
          <strong>{credit.name}</strong> by {credit.by}: {credit.what}
          <span class="mono muted">{credit.url}</span>
        </li>
      {/each}
    </ul>
    <p class="muted">
      Made with Tauri, Rust and Svelte. Cartload is a from-scratch implementation and shares no code with hakchi2 CE.
    </p>
    {#if app.version}
      <details class="crates">
        <summary>Linked crates</summary>
        <ul class="muted">
          {#each app.version.crates as crate (crate)}
            <li class="num">{crate}</li>
          {/each}
        </ul>
      </details>
    {/if}
  </section>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 768px;
  }

  .card {
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    border: 0;
  }

  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .option-icon {
    display: inline-flex;
    color: var(--text-muted);
  }

  .path {
    user-select: text;
    overflow-wrap: anywhere;
  }

  h3 {
    margin-top: var(--space-2);
  }

  .key {
    display: flex;
    gap: var(--space-2);
  }

  .key input {
    flex: 1;
  }

  .about:focus-visible {
    outline-offset: 0;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .identity-text {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .wordmark {
    display: inline-flex;
    color: var(--text);
  }

  .credits {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .credits .mono {
    user-select: text;
  }

  .crates {
    font-size: var(--text-md);
  }

  .crates ul {
    margin: var(--space-1) 0 0;
    padding-left: 20px;
  }
</style>
