<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { app, preferences, type Theme } from "../state";

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
</style>
