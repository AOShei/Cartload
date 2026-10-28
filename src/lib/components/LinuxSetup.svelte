<script lang="ts">
  import { onMount } from "svelte";
  import { setupInstall, setupStatus } from "../api/commands";
  import { AppError, inTauri } from "../api/tauri";
  import type { SetupStatus } from "../api/types";
  import { toasts } from "../state";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  /* The host setup Linux needs to talk to the console (USB access, the hakchi0 interface,
     its network profile), with a dialog that checks it and installs what is missing. */
  interface Props {
    /** Show nothing while the setup is complete (the Console page's banner). */
    onlyWhenNeeded?: boolean;
  }

  let { onlyWhenNeeded = false }: Props = $props();
  let setup = $state<SetupStatus | null>(null);
  let busy = $state(false);
  let open = $state(false);
  let checking = $state(false);

  onMount(() => {
    if (inTauri) {
      void setupStatus()
        .then((status) => (setup = status))
        .catch((error: unknown) => toasts.error(`Could not read the system setup: ${AppError.from(error).message}`));
    }
  });

  /** Runs the checks again and shows the result in the dialog. */
  async function check(): Promise<void> {
    open = true;
    checking = true;
    try {
      setup = await setupStatus();
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(`Could not read the system setup: ${e.message}`, e.detail);
    } finally {
      checking = false;
    }
  }

  async function install(): Promise<void> {
    busy = true;
    try {
      setup = await setupInstall();
      toasts.success(setup.complete ? "Linux setup installed. Reconnect the console." : "Installed what could be installed.");
    } catch (error: unknown) {
      const e = AppError.from(error);
      toasts.error(e.message, e.detail);
    } finally {
      busy = false;
    }
  }
</script>

{#if setup?.applies && !(onlyWhenNeeded && setup.complete)}
  <section class="card setup" class:attention={!setup.complete} aria-labelledby="setup-heading">
    <h2 id="setup-heading">Linux setup</h2>
    {#if setup.complete}
      <span class="status ok"><Icon name="check" size={16} /> Ready</span>
      <button type="button" class="btn btn-quiet btn-sm" onclick={() => void check()}>Check again</button>
    {:else}
      <span class="status warn"><Icon name="warning" size={16} /> This computer needs a one-time setup to reach the console</span>
      <button type="button" class="btn btn-sm" onclick={() => void check()}>Check setup…</button>
    {/if}
  </section>
{/if}

{#if setup}
  <Dialog {open} title="Linux setup" width="560px" onclose={() => (open = false)}>
    {#if checking}
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
        <button type="button" class="btn btn-primary" onclick={() => void install()} disabled={busy || checking || !setup?.can_install}>
          {#if busy}<span class="spinner on-accent" aria-hidden="true"></span>{/if}
          {busy ? "Installing…" : "Install"}
        </button>
      {/if}
      <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (open = false)}>Close</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .setup {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-5);
  }

  .setup h2 {
    flex: none;
  }

  .setup.attention {
    border-left: 4px solid var(--warn);
  }

  .status {
    flex: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
  }

  .status.ok {
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

  .small {
    font-size: var(--text-sm);
  }

  .spinner.on-accent {
    border-color: color-mix(in srgb, var(--on-accent) 35%, transparent);
    border-top-color: var(--on-accent);
  }
</style>
