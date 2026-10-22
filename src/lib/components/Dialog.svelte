<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  /* A modal editor on the native <dialog>: the browser traps focus inside it (the rest of
     the page is inert), Escape fires `cancel`, and focus returns to the element that opened
     it when it closes. `docs/design.md` reserves dialogs for editors. */
  interface Props {
    open: boolean;
    title: string;
    /** Width of the card; the height follows the content and scrolls past the window. */
    width?: string;
    /** Asked to close: Escape, the close button, a click on the backdrop. */
    onclose: () => void;
    children: Snippet;
    actions?: Snippet;
  }

  let { open, title, width = "560px", onclose, children, actions }: Props = $props();
  let element: HTMLDialogElement | undefined = $state();
  const id = $props.id();

  $effect(() => {
    if (!element) {
      return;
    }
    if (open && !element.open) {
      element.showModal();
      // An editor marks the control that should take focus; otherwise the first control of
      // the body or the actions does, so a screen reader lands on the content rather than on
      // the header's close button (which the browser would focus first).
      const first =
        element.querySelector<HTMLElement>("[data-autofocus]") ??
        element.querySelector<HTMLElement>(
          ".content button:not([disabled]), .content input:not([disabled]), .content select:not([disabled]), .content textarea:not([disabled]), .content [tabindex='0'], footer button:not([disabled]):not(.btn-danger), footer button:not([disabled])",
        );
      first?.focus();
    } else if (!open && element.open) {
      element.close();
    }
  });

  function oncancel(event: Event): void {
    event.preventDefault();
    onclose();
  }

  /** The browser closed it itself (a second Escape without activation): follow. */
  function onclosed(): void {
    if (open) {
      onclose();
    }
  }

  function onclick(event: MouseEvent): void {
    if (event.target === element) {
      onclose();
    }
  }
</script>

<dialog bind:this={element} oncancel={oncancel} onclose={onclosed} onclick={onclick} aria-labelledby="{id}-title" style:--dialog-width={width}>
  <div class="card body">
    <header>
      <h2 id="{id}-title">{title}</h2>
      <button type="button" class="icon-btn" onclick={onclose} aria-label="Close" title="Close (Escape)">
        <Icon name="close" size={16} />
      </button>
    </header>
    <div class="content">{@render children()}</div>
    {#if actions}
      <footer>{@render actions()}</footer>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(var(--dialog-width), calc(100vw - 2 * var(--space-5)));
    max-height: calc(100vh - 2 * var(--space-5));
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    overflow: visible;
  }

  dialog::backdrop {
    background: color-mix(in srgb, var(--bg) 60%, transparent);
  }

  dialog[open] {
    animation: dialog-in var(--motion) var(--ease-out);
  }

  @keyframes dialog-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .body {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 2 * var(--space-5));
    box-shadow: var(--shadow-float);
  }

  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-5);
    border-bottom: 1px solid var(--border);
  }

  header h2 {
    flex: 1;
    min-width: 0;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    font-size: var(--text-md);
  }

  footer {
    flex: none;
    display: flex;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-5);
    border-top: 1px solid var(--border);
  }
</style>
