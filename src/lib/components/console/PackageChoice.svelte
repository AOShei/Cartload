<script lang="ts">
  /* Which kernel package the Console page installs: Cartload's system (the default) or
     hakchi v1.0.4-126. The choice is a preference; the status follows it. */
  import { packageStore } from "../../state";
  import { preferences } from "../../state/preferences.svelte";

  let { disabled = false }: { disabled?: boolean } = $props();

  const choices = $derived(packageStore.status?.choices ?? []);
</script>

{#if choices.length > 1}
  <select
    class="choice num"
    aria-label="Kernel package"
    value={packageStore.status?.choice ?? preferences.kernelPackage}
    {disabled}
    onchange={(e) => void packageStore.choose(e.currentTarget.value)}
  >
    {#each choices as choice, i (choice.id)}
      <option value={choice.id}>{choice.label}{i === 0 ? " (default)" : ""}</option>
    {/each}
  </select>
{:else}
  <strong class="num">{packageStore.label()}</strong>
{/if}

<style>
  .choice {
    font: inherit;
    font-weight: 600;
    max-width: 100%;
  }
</style>
