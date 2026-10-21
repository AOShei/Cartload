<script lang="ts">
  import { consoleStore } from "../state";

  const title = $derived(consoleStore.report?.title ?? "Looking for the console…");
</script>

<div class="page">
  <section class="card">
    <h2>Kernel install is not part of this build yet.</h2>
    <p class="muted">
      This page will guide you through putting the console into FEL mode, installing hakchi and
      checking the result, step by step and with the live log beneath.
    </p>
    <p>Console right now: <strong>{title}</strong></p>
  </section>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .card {
    padding: var(--space-5) var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 64ch;
  }
</style>
