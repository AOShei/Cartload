<script lang="ts">
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { FAST, NORMAL, ms } from "../motion";
  import { toasts } from "../state";
</script>

<div class="toaster" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast kind-{toast.kind}"
      role={toast.kind === "error" ? "alert" : "status"}
      in:fly={{ y: -8, duration: ms(NORMAL), easing: cubicOut }}
      out:fade={{ duration: ms(FAST) }}
      animate:flip={{ duration: ms(NORMAL) }}
    >
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
      {#if toast.action}
        <button type="button" class="btn btn-sm" onclick={() => toasts.act(toast.id)}>
          {toast.action.label}
        </button>
      {/if}
      <button
        type="button"
        class="icon-btn icon-btn-sm"
        aria-label="Dismiss notification"
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
    width: min(416px, calc(100vw - 2 * var(--space-4)));
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
    padding-top: 2px;
  }

  details {
    margin-top: var(--space-1);
    font-size: var(--text-sm);
  }

  pre {
    margin: var(--space-1) 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    color: var(--text-muted);
  }
</style>
