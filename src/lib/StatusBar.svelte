<script lang="ts">
  import type { AppVersion } from "./version";

  interface Props {
    version: AppVersion | null;
    error: string | null;
  }

  let { version, error }: Props = $props();
</script>

<footer class="status" role="status">
  {#if version}
    <span>Cartload {version.app}</span>
    <span class="muted" title={version.crates.join("\n")}>
      {version.crates.length} crates
    </span>
  {:else if error}
    <span class="error">Version unavailable: {error}</span>
  {:else}
    <span class="muted">Loading version…</span>
  {/if}
</footer>

<style>
  .status {
    display: flex;
    gap: 1rem;
    align-items: center;
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
    background: var(--surface-raised);
    border-top: 1px solid var(--border);
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
