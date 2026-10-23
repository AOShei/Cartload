<script lang="ts">
  import { onMount, tick } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import {
    diagnosticsReport,
    diagnosticsSave,
    logFile,
    logReveal,
    setupInstall,
    setupStatus,
    tgdbKeyGet,
    tgdbKeySet,
  } from "../api/commands";
  import { AppError, inTauri } from "../api/tauri";
  import type { KeyStatus, LogFileInfo, SetupStatus } from "../api/types";
  import AppMark from "../components/AppMark.svelte";
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import Wordmark from "../components/Wordmark.svelte";
  import { app, consoleStore, flowStore, formatBytes, libraryStore, modulesStore, navigation, preferences, toasts, type Theme } from "../state";

  let keyStatus = $state<KeyStatus | null>(null);
  let keyLoading = $state(false);
  let keyValue = $state("");
  let keyBusy = $state(false);
  let confirmKeyRemoval = $state(false);
  let about: HTMLElement | undefined = $state();
  let setup = $state<SetupStatus | null>(null);
  let setupBusy = $state(false);
  let setupOpen = $state(false);
  let setupChecking = $state(false);
  let log = $state<LogFileInfo | null>(null);
  /** A console running a hakchi kernel the app can talk to: modules install there. */
  const consoleReady = $derived.by((): boolean => {
    const status = consoleStore.report?.status;
    return status?.state === "connected" && status.info.can_interact && (consoleStore.report?.supported ?? true);
  });
  let diagnosticsBusy = $state(false);

  onMount(() => {
    void libraryStore.start();
    void modulesStore.start();
    if (inTauri) {
      void setupStatus()
        .then((status) => (setup = status))
        .catch((error: unknown) => toasts.error(`Could not read the system setup: ${AppError.from(error).message}`));
      void logFile()
        .then((info) => (log = info))
        .catch(() => (log = null));
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

  /** Runs the checks again and shows the result in the dialog. */
  async function checkSetup(): Promise<void> {
    setupOpen = true;
    setupChecking = true;
    try {
      setup = await setupStatus();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the system setup: ${e.message}`, e.detail);
    } finally {
      setupChecking = false;
    }
  }

  async function installSetup(): Promise<void> {
    setupBusy = true;
    try {
      setup = await setupInstall();
      toasts.success(setup.complete ? "Linux setup installed. Reconnect the console." : "Installed what could be installed.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      setupBusy = false;
    }
  }

  async function copyDiagnostics(): Promise<void> {
    diagnosticsBusy = true;
    try {
      const text = await diagnosticsReport();
      await navigator.clipboard.writeText(text);
      toasts.success("Diagnostics copied. Paste them into the bug report.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not copy the diagnostics: ${e.message}`, e.detail);
    } finally {
      diagnosticsBusy = false;
    }
  }

  async function saveDiagnostics(): Promise<void> {
    const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
    let path: string | null;
    try {
      path = await save({
        title: "Save diagnostics",
        defaultPath: `cartload-diagnostics-${stamp}.txt`,
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
    } catch {
      toasts.info("The save dialog needs the desktop app.");
      return;
    }
    if (!path) {
      return;
    }
    diagnosticsBusy = true;
    try {
      await diagnosticsSave(path);
      toasts.success("Diagnostics saved.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      diagnosticsBusy = false;
    }
  }

  async function revealLog(): Promise<void> {
    try {
      await logReveal();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    }
  }

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
    { id: "system", label: "System", hint: "" },
    { id: "light", label: "Light", hint: "" },
    { id: "dark", label: "Dark", hint: "" },
  ];

  const credits: { name: string; by: string; url: string; what: string }[] = [
    { name: "hakchi", by: "madmonkey", url: "github.com/madmonkey1907/hakchi", what: "the kernel and the device-side tools Cartload installs and talks to (GPL-3)." },
    { name: "hakchi2 CE", by: "TeamShinkansen", url: "github.com/TeamShinkansen/Hakchi2-CE", what: "the reference for the console protocol, the FEL sequence and the sync (GPL-3)." },
    { name: "RetroArch", by: "the Libretro team", url: "retroarch.com", what: "the emulator frontend and cores the Emulators section installs (GPL-3), packaged for the console by KMFDManic." },
    { name: "TheGamesDB", by: "the TheGamesDB community", url: "thegamesdb.net", what: "cover art and game metadata, fetched with your own API key." },
  ];
</script>

<div class="page">
  <div class="column">
  <section class="card row-card" aria-labelledby="theme-heading">
    <h2 id="theme-heading">Appearance</h2>
    <div class="segmented" role="radiogroup" aria-labelledby="theme-heading">
      {#each themes as theme (theme.id)}
        <button
          type="button"
          role="radio"
          aria-checked={preferences.theme === theme.id}
          class:active={preferences.theme === theme.id}
          onclick={() => preferences.setTheme(theme.id)}
        >
          <Icon name={theme.id === "dark" ? "moon" : "sun"} size={14} />
          {theme.label}
        </button>
      {/each}
    </div>
  </section>

  <section class="card" aria-labelledby="log-heading">
    <div class="head">
      <h2 id="log-heading">Activity log</h2>
      <label class="option">
        <input type="checkbox" checked={preferences.showDebugLog} onchange={(e) => preferences.setShowDebugLog(e.currentTarget.checked)} />
        <span>Show details</span>
      </label>
    </div>
    <p class="muted small">Cartload keeps a log of what it does with the console. Diagnostics bundle that log with your system details for a bug report; they never include your TheGamesDB key.</p>
    <div class="actions">
      <button type="button" class="btn" onclick={() => void copyDiagnostics()} disabled={diagnosticsBusy || !inTauri}>
        {#if diagnosticsBusy}<span class="spinner" aria-hidden="true"></span>{/if}
        Copy diagnostics
      </button>
      <button type="button" class="btn btn-quiet" onclick={() => void saveDiagnostics()} disabled={diagnosticsBusy || !inTauri}>Save diagnostics…</button>
      {#if log?.path}
        <button type="button" class="btn btn-quiet" onclick={() => void revealLog()} title={log.path}>Open log folder</button>
      {/if}
    </div>
  </section>

  {#if setup?.applies}
    <section class="card row-card" aria-labelledby="setup-heading">
      <h2 id="setup-heading">Linux setup</h2>
      {#if setup.complete}
        <span class="status ok"><Icon name="check" size={16} /> Ready</span>
        <button type="button" class="btn btn-quiet btn-sm" onclick={() => void checkSetup()}>Check again</button>
      {:else}
        <span class="status warn"><Icon name="warning" size={16} /> Needs attention</span>
        <button type="button" class="btn btn-sm" onclick={() => void checkSetup()}>Check setup…</button>
      {/if}
    </section>
  {/if}

  <section class="card grow" aria-labelledby="modules-heading">
    <h2 id="modules-heading">Emulators</h2>
    <p class="muted small">RetroArch and its cores install into hakchi's storage on the console. The built-in emulators and the menu stay untouched.</p>
    {#if modulesStore.status}
      <ul class="modules">
        {#each modulesStore.status.modules as module (module.id)}
          {@const progress = modulesStore.progress[module.id]}
          {@const downloading = modulesStore.downloading.includes(module.id)}
          <li>
            <div class="module-text">
              <strong>{module.title}</strong>
              <span class="muted small">
                {module.purpose}
                {#if downloading && progress}
                  · Downloading {formatBytes(progress.done)}{progress.total ? ` of ${formatBytes(progress.total)}` : ""}
                {:else if module.installed === true}
                  · <span class="ok-text">Installed</span>
                {:else if !module.cached}
                  · {formatBytes(module.approx_bytes)}
                {/if}
              </span>
            </div>
            <button
              type="button"
              class="btn btn-sm"
              onclick={() => void modulesStore.install(module.id === "retroarch" ? ["retroarch"] : ["retroarch", module.id])}
              disabled={downloading || flowStore.running || module.installed === true || !consoleReady}
              title={consoleReady ? "Downloads the module and reboots the console once" : "Connect a console running hakchi"}
            >
              {#if downloading}<span class="spinner" aria-hidden="true"></span>{/if}
              {module.installed === true ? "Installed" : "Install"}
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted"><span class="spinner" aria-hidden="true"></span> Reading the modules…</p>
    {/if}
    <p class="muted small foot">Modules come from KMFDManic's Mod Hub. Installing needs a connected console running hakchi and reboots it once.</p>
  </section>
  </div>

  <div class="column">
  <section class="card about grow" aria-labelledby="about-heading" bind:this={about} tabindex="-1">
    <div class="identity">
      <AppMark size={64} />
      <div class="identity-text">
        <h2 id="about-heading" class="visually-hidden">About Cartload</h2>
        <span class="wordmark"><Wordmark height={26} label="Cartload" /></span>
        <p class="muted small">
          {#if app.version}
            Version <span class="num">{app.version.app}</span>{app.version.debug ? ", debug build" : ""} · GPL-3.0-or-later · <span class="mono">github.com/AOShei/Cartload</span>
          {:else if app.versionError}
            Version unknown ({app.versionError})
          {:else}
            Reading the version…
          {/if}
        </p>
      </div>
    </div>
    <h3>Built on</h3>
    <ul class="credits">
      {#each credits as credit (credit.name)}
        <li>
          <strong>{credit.name}</strong> by {credit.by}: {credit.what}
          <span class="mono muted">{credit.url}</span>
        </li>
      {/each}
    </ul>
    <p class="muted small foot">Made with Tauri, Rust and Svelte. A from-scratch implementation; shares no code with hakchi2 CE.</p>
  </section>

  <section class="card" aria-labelledby="library-heading">
    <div class="head">
      <h2 id="library-heading">Library</h2>
      <span class="mono path muted small" title={libraryStore.view?.root ?? ""}>{libraryStore.view?.root ?? "the user data directory"}</span>
    </div>
    <p class="muted small">
      TheGamesDB key for cover art (free at thegamesdb.net):
      {#if keyLoading}checking…{:else if keyStatus?.environment}set by CARTLOAD_TGDB_KEY{:else if keyStatus?.stored}stored{:else}none yet{/if}{#if keyStatus?.remaining !== null && keyStatus?.remaining !== undefined}, <span class="num">{keyStatus.remaining}</span> requests left this month{/if}.
    </p>
    <form class="key" onsubmit={(e) => { e.preventDefault(); void saveKey(false); }}>
      <input type="password" bind:value={keyValue} placeholder={keyStatus?.stored ? "Replace the stored key" : "Paste the key"} aria-label="TheGamesDB API key" autocomplete="off" disabled={keyBusy} />
      <button type="submit" class="btn" disabled={keyBusy || !keyValue.trim()}>
        {#if keyBusy}<span class="spinner" aria-hidden="true"></span>{/if}
        Store
      </button>
      {#if keyStatus?.stored}
        <button type="button" class="btn btn-quiet" disabled={keyBusy} onclick={() => (confirmKeyRemoval = true)}>Remove…</button>
      {/if}
    </form>
  </section>
  </div>
</div>

<Dialog open={confirmKeyRemoval} title="Remove the TheGamesDB key?" width="440px" onclose={() => (confirmKeyRemoval = false)}>
  <p>Fetching art stops working until a key is stored again.</p>
  {#snippet actions()}
    <button type="button" class="btn btn-danger" disabled={keyBusy} onclick={() => saveKey(true)}>Remove key</button>
    <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (confirmKeyRemoval = false)}>Keep it</button>
  {/snippet}
</Dialog>

{#if setup}
  <Dialog open={setupOpen} title="Linux setup" width="560px" onclose={() => (setupOpen = false)}>
    {#if setupChecking}
      <p class="muted"><span class="spinner" aria-hidden="true"></span> Checking…</p>
    {:else}
      <ul class="checks">
        <li class:ok={setup.usb_rule}><Icon name={setup.usb_rule ? "check" : "warning"} size={16} /><span>USB access rule</span></li>
        <li class:ok={setup.name_rule}><Icon name={setup.name_rule ? "check" : "warning"} size={16} /><span>hakchi0 interface name</span></li>
        {#if setup.network_manager}
          <li class:ok={setup.network_profile}><Icon name={setup.network_profile ? "check" : "warning"} size={16} /><span>Link-local network profile</span></li>
        {:else}
          <li><Icon name="warning" size={16} /><span>NetworkManager not found: give hakchi0 a link-local address yourself.</span></li>
        {/if}
        {#if setup.firewalld}
          {#if setup.firewall_zone === "trusted"}
            <li class="ok"><Icon name="check" size={16} /><span>Firewall: hakchi0 trusted</span></li>
          {:else if setup.firewall_zone}
            <li><Icon name="warning" size={16} /><span>Firewall: hakchi0 is in the {setup.firewall_zone} zone</span></li>
          {:else}
            <li class="ok"><Icon name="dash" size={16} /><span>Firewall: checked once the console is connected</span></li>
          {/if}
        {/if}
      </ul>
      {#if !setup.complete}
        <p class="muted small">Install writes the missing files and asks for your password.{#if !setup.can_install} pkexec was not found, so install them by hand (docs/hardware-tests.md).{/if}</p>
      {/if}
    {/if}
    {#snippet actions()}
      {#if !setup?.complete}
        <button type="button" class="btn btn-primary" onclick={() => void installSetup()} disabled={setupBusy || setupChecking || !setup?.can_install}>
          {#if setupBusy}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
          {setupBusy ? "Installing…" : "Install"}
        </button>
      {/if}
      <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (setupOpen = false)}>Close</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  /* Narrow: one scrolling column. Wide (1100 px and up): the settings on the left and the
     About card on the right, each column scrolling on its own. */
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 768px;
    overflow: auto;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  @media (min-width: 1100px) {
    .page {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(360px, 1fr);
      grid-template-rows: minmax(0, 1fr);
      gap: var(--space-5);
      max-width: none;
      overflow: hidden;
    }

    /* Both columns fill the page height, so their last cards end on the same line. */
    .column {
      height: 100%;
      overflow: auto;
    }

    .grow {
      flex: 1 0 auto;
    }

  }

  .card {
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  /* A heading with its control on the same line. */
  .head {
    display: flex;
  }

  .row-card,
  .head {
    flex-direction: row;
    align-items: center;
    gap: var(--space-3);
  }

  .row-card h2,
  .head h2 {
    flex: 1;
  }

  .head .path {
    flex: 2;
    min-width: 0;
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .segmented {
    display: inline-flex;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    overflow: hidden;
  }

  .segmented button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border: 0;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }

  .segmented button + button {
    border-left: 1px solid var(--border-strong);
  }

  .segmented button.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
  }

  .status.ok,
  .ok-text {
    color: var(--ok);
  }

  .status.warn {
    color: var(--warn);
  }

  .checks {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .checks li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-muted);
  }

  .checks li.ok {
    color: var(--text);
  }

  .checks li.ok :global(svg) {
    color: var(--ok);
  }

  .option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .path {
    user-select: text;
    overflow-wrap: anywhere;
  }

  h3 {
    margin-top: var(--space-1);
  }

  .small {
    font-size: var(--text-sm);
  }

  .modules {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .modules li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
  }

  .module-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .modules .btn {
    flex: none;
    min-width: 88px;
  }

  .key,
  .actions {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .key input {
    flex: 1;
  }

  /* Sits at the bottom of a card that grows to fill its column. */
  .foot {
    margin-top: auto;
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
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
    gap: var(--space-3);
    line-height: 1.5;
  }

  .credits .mono {
    display: block;
  }

  .credits .mono {
    user-select: text;
  }

</style>
