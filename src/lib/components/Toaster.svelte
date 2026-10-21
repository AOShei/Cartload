<script lang="ts">
  import Icon from "./Icon.svelte";
  import { toasts } from "../state";
</script>

<div class="toaster" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div class="toast kind-{toast.kind}" role={toast.kind === "error" ? "alert" : "status"}>
      <span class="icon">
        <Icon name={toast.kind === "error" ? "warning" : toast.kind === "success" ? "check" : "info"} />
      </span>
      <div class="body">
        <p>{toast.message}</p>
        {#if toast.detail}
          <details>
            <summary>Details</summary>
            <pre>{toast.detail}</pre>
          </details>
        {/if}
      </div>
      <button
        type="button"
        class="icon-btn"
        aria-label="Dismiss"
        onclick={() => toasts.dismiss(toast.id)}
      >
        <Icon name="close" size={16} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    top: var(--space-4);
    right: var(--space-4);
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: min(26rem, calc(100vw - 2 * var(--space-4)));
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius-m);
    background: var(--surface);
    box-shadow: var(--shadow-float);
    pointer-events: auto;
    animation: slide-in 160ms ease-out;
  }

  .kind-error {
    border-left-color: var(--danger);
  }

  .kind-error .icon {
    color: var(--danger);
  }

  .kind-success {
    border-left-color: var(--ok);
  }

  .kind-success .icon {
    color: var(--ok);
  }

  .kind-info .icon {
    color: var(--accent);
  }

  .icon {
    display: inline-flex;
    margin-top: 1px;
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  details {
    margin-top: var(--space-1);
    font-size: 0.8rem;
  }

  pre {
    margin: var(--space-1) 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    color: var(--text-muted);
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
