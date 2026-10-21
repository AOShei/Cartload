<script lang="ts">
  import { onMount } from "svelte";
  import { tgdbKeyGet, tgdbKeySet } from "../api/commands";
  import { AppError, inTauri } from "../api/tauri";
  import type { KeyStatus } from "../api/types";
  import Icon from "../components/Icon.svelte";
  import { app, libraryStore, preferences, toasts, type Theme } from "../state";

  let keyStatus = $state<KeyStatus | null>(null);
  let keyValue = $state("");
  let keyBusy = $state(false);

  onMount(() => {
    void libraryStore.start();
    if (inTauri) {
      void tgdbKeyGet().then((status) => (keyStatus = status)).catch(() => {});
    }
  });

  async function saveKey(clear: boolean): Promise<void> {
    if (!inTauri) {
      toasts.info("Keys are stored inside the desktop app only.");
      return;
    }
    keyBusy = true;
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
    <p class="muted">Games are kept in <span class="mono">{libraryStore.view?.root ?? "the user data directory"}</span>.</p>
    <h3>TheGamesDB key</h3>
    <p class="muted">
      Cover art and metadata come from TheGamesDB, which needs a free API key from thegamesdb.net.
      {#if keyStatus?.environment}
        The CARTLOAD_TGDB_KEY variable is set and is used.
      {:else if keyStatus?.stored}
        A key is stored.
      {:else}
        No key yet.
      {/if}
      {#if keyStatus?.remaining !== null && keyStatus?.remaining !== undefined}
        {keyStatus.remaining} requests left this month.
      {/if}
    </p>
    <form class="key" onsubmit={(e) => { e.preventDefault(); void saveKey(false); }}>
      <input type="password" bind:value={keyValue} placeholder={keyStatus?.stored ? "Replace the stored key" : "Paste the key"} aria-label="TheGamesDB API key" autocomplete="off" />
      <button type="submit" class="btn" disabled={keyBusy || !keyValue.trim()}>Store</button>
      {#if keyStatus?.stored}
        <button type="button" class="btn btn-quiet" disabled={keyBusy} onclick={() => saveKey(true)}>Remove</button>
      {/if}
    </form>
  </section>

  <section class="card" aria-labelledby="about-heading">
    <h2 id="about-heading">About</h2>
    {#if app.version}
      <p>Cartload {app.version.app}{app.version.debug ? " (debug build)" : ""}</p>
      <ul class="crates muted">
        {#each app.version.crates as crate (crate)}
          <li class="num">{crate}</li>
        {/each}
      </ul>
    {:else}
      <p class="muted">Version information is only available inside the desktop app.</p>
    {/if}
    <p class="muted">GPL-3.0-or-later. Built on hakchi by madmonkey and hakchi2 CE by TeamShinkansen.</p>
  </section>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 48rem;
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

  .crates {
    margin: 0;
    padding-left: 1.2rem;
    font-size: 0.85rem;
  }

  h3 {
    margin-top: var(--space-2);
    font-size: 0.95rem;
  }

  .key {
    display: flex;
    gap: var(--space-2);
  }

  .key input {
    flex: 1;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--surface);
  }
</style>
