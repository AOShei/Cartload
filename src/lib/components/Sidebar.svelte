<script lang="ts">
  import Cartridge from "./Cartridge.svelte";
  import Icon from "./Icon.svelte";
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
  <div class="brand" aria-label="Cartload">
    <span class="glyph"><Cartridge size={28} /></span>
    <span class="wordmark">Cartload</span>
  </div>
  <ul role="list">
    {#each PAGES as page, i (page.id)}
      <li>
        <button
          type="button"
          class:active={page.id === navigation.page}
          aria-current={page.id === navigation.page ? "page" : undefined}
          title={`${page.label} (Ctrl+${i + 1})`}
          bind:this={buttons[i]}
          onclick={() => select(page.id)}
          onkeydown={onkeydown}
        >
          <Icon name={page.icon} />
          <span>{page.label}</span>
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 13rem;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-right: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-2) var(--space-5);
  }

  .glyph {
    color: var(--accent);
    display: inline-flex;
  }

  .wordmark {
    font-weight: 700;
    font-size: 1.1rem;
    letter-spacing: 0.01em;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: 0.55rem 0.75rem;
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text-muted);
    text-align: left;
    cursor: pointer;
  }

  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  button.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-weight: 600;
  }

  button.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0.5rem;
    bottom: 0.5rem;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
</style>
