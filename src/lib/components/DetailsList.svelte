<script lang="ts">
  import type { Detail } from "../api/types";

  interface Props {
    details: Detail[];
  }

  let { details }: Props = $props();
</script>

<dl class="details">
  {#each details as detail (detail.label)}
    <dt>{detail.label}</dt>
    <dd class="num">{detail.value}</dd>
  {/each}
</dl>

<style>
  .details {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--space-2) var(--space-5);
    margin: 0;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    user-select: text;
    overflow-wrap: anywhere;
  }
</style>
