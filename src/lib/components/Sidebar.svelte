<script lang="ts">
  import Cartridge from "./Cartridge.svelte";
  import Icon from "./Icon.svelte";
  import Wordmark from "./Wordmark.svelte";
  import { PAGES, navigation, type PageId } from "../state";

  let buttons: HTMLButtonElement[] = $state([]);

  function select(page: PageId): void {
    navigation.go(page);
  }

  /** Up / Down / Home / End move the focus between the pages; Enter and Space activate. */
  function onkeydown(event: KeyboardEvent): void {
    const index = buttons.findIndex((b) => b === document.activeElement);
    if (index < 0) {
      return;
    }
    let target: number | null = null;
    switch (event.key) {
      case "ArrowDown":
        target = (index + 1) % buttons.length;
        break;
      case "ArrowUp":
        target = (index - 1 + buttons.length) % buttons.length;
        break;
      case "Home":
        target = 0;
        break;
      case "End":
        target = buttons.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    buttons[target]?.focus();
  }
</script>

<nav class="sidebar" aria-label="Pages">
  <button
    type="button"
    class="brand"
    aria-label="About Cartload"
    title="About Cartload"
    onclick={() => navigation.go("settings", "about")}
  >
    <span class="glyph"><Cartridge size={28} /></span>
    <span class="wordmark"><Wordmark height={16} /></span>
  </button>
  <ul role="list">
    {#each PAGES as page, i (page.id)}
      <li>
        <button
          type="button"
          class:active={page.id === navigation.page}
          aria-current={page.id === navigation.page ? "page" : undefined}
          aria-label={page.label}
          title={`${page.hint} (Ctrl+${i + 1})`}
          bind:this={buttons[i]}
          onclick={() => select(page.id)}
          onkeydown={onkeydown}
        >
          <Icon name={page.icon} />
          <span class="label">{page.label}</span>
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 200px;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-right: 1px solid var(--border);
    transition: width var(--motion) var(--ease-out);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control-lg);
    margin: 0 0 var(--space-4);
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .brand:hover {
    background: var(--surface-hover);
  }

  .glyph {
    color: var(--accent);
    display: inline-flex;
  }

  .wordmark {
    display: inline-flex;
    color: var(--text);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li button {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: var(--control-lg);
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text-muted);
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  li button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  li button.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-weight: var(--weight-semibold);
  }

  li button.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: var(--space-2);
    bottom: var(--space-2);
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }

  /* Narrow windows: icons only, the labels stay as tooltips and accessible names. */
  @media (max-width: 1023px) {
    .sidebar {
      width: 64px;
      padding: var(--space-3) var(--space-2);
    }

    .brand {
      justify-content: center;
      padding: 0;
    }

    .wordmark,
    .label {
      display: none;
    }

    li button {
      justify-content: center;
      padding: 0;
    }
  }
</style>
