<script lang="ts">
  import { modulesStore } from "../state";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  /* Asks before installing a module that carries a warning (the N64 core). Opened by
     `modulesStore.install`, from Settings or from the sync's warning. */
  const pending = $derived(modulesStore.pending);
</script>

<Dialog open={pending !== null} title="Install at your own risk?" width="520px" onclose={() => (modulesStore.pending = null)}>
  {#if pending}
    {#each pending.warnings as warning (warning.title)}
      <div class="warning">
        <Icon name="warning" size={18} />
        <div>
          <strong>{warning.title}</strong>
          <p>{warning.text}</p>
        </div>
      </div>
    {/each}
    <p class="muted small">It installs into hakchi's storage like the other modules. Your other games and emulators are not affected.</p>
  {/if}
  {#snippet actions()}
    <button type="button" class="btn btn-primary" onclick={() => void modulesStore.confirmPending()}>Install anyway</button>
    <button type="button" class="btn btn-quiet" data-autofocus onclick={() => (modulesStore.pending = null)}>Cancel</button>
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

  .warning div {
    color: var(--text);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .warning p {
    margin: 0;
  }

  .small {
    font-size: var(--text-sm);
  }
</style>
