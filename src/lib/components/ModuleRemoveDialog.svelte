<script lang="ts">
  import { modulesStore } from "../state";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  /* Asks about removing an emulator module, opened by `modulesStore.requestRemove` from
     Settings. While games in the library still need the module it explains that they go
     first and offers to show them in the Library; otherwise it asks to confirm. */
  const removal = $derived(modulesStore.removal);
  const blocked = $derived(removal !== null && removal.games.length > 0);
  const SHOWN = 6;

  function close(): void {
    modulesStore.removal = null;
  }
</script>

<Dialog
  open={removal !== null}
  title={removal?.unmanaged ? "Cartload cannot remove this" : blocked ? "Remove these games first" : `Remove ${removal?.title ?? ""}?`}
  width="520px"
  onclose={close}
>
  {#if removal}
    {#if removal.unmanaged}
      <p>{removal.title} is on the console, but not under the name Cartload installs it with, so there is no uninstall record for Cartload to run. Remove it with the tool that installed it (hakchi2 CE's Uninstall mods).</p>
    {:else if blocked}
      {@const count = removal.games.length}
      <div class="warning">
        <Icon name="warning" size={18} />
        <p>
          {count} game{count === 1 ? "" : "s"} in the library {count === 1 ? "runs" : "run"} on {removal.title}{removal.id === "retroarch" ? " or one of its cores" : ""}.
          Remove {count === 1 ? "it" : "them"} from the library, or switch {count === 1 ? "it" : "them"} to the built-in emulator where the console has one, then sync. After that you can remove the emulator.
        </p>
      </div>
      <ul class="games">
        {#each removal.games.slice(0, SHOWN) as view (view.game.code)}
          <li>{view.game.name} <span class="muted">{view.system_name}</span></li>
        {/each}
        {#if count > SHOWN}
          <li class="muted">and {count - SHOWN} more</li>
        {/if}
      </ul>
    {:else}
      <p>
        Cartload runs the module's uninstall script on the console and reboots it once.
        {#if removal.alsoRemoved.length > 0}
          Every core needs RetroArch, so the installed cores go too: {removal.alsoRemoved.join(", ")}.
        {/if}
      </p>
      <p class="muted small">No game in the library uses it. If you removed games without syncing since, sync first so the console's menu does not keep games that can no longer start.</p>
    {/if}
  {/if}
  {#snippet actions()}
    {#if removal?.unmanaged}
      <button type="button" class="btn btn-primary" data-autofocus onclick={close}>OK</button>
    {:else if blocked}
      <button type="button" class="btn btn-primary" data-autofocus onclick={() => modulesStore.showBlockingGames()}>Show these games</button>
      <button type="button" class="btn btn-quiet" onclick={close}>Close</button>
    {:else}
      <button type="button" class="btn btn-danger" onclick={() => void modulesStore.confirmRemove()}>Remove</button>
      <button type="button" class="btn btn-quiet" data-autofocus onclick={close}>Cancel</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .warning {
    display: flex;
    gap: var(--space-3);
    align-items: flex-start;
    color: var(--warn);
  }

  .warning > :global(svg) {
    flex: none;
    margin-top: 2px;
  }

  .warning p {
    margin: 0;
    color: var(--text);
  }

  .games {
    margin: var(--space-3) 0 0;
    padding-left: var(--space-5);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .small {
    font-size: var(--text-sm);
  }
</style>
