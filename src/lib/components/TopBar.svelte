<script lang="ts">
  import Cartridge from "./Cartridge.svelte";
  import Icon from "./Icon.svelte";
  import Wordmark from "./Wordmark.svelte";
  import { PAGES, navigation } from "../state";

  /* The app's header: the brand (opens About) and the three pages as tabs. A top bar leaves
     the whole width to the pages, which a sidebar for three entries did not. */
  let buttons: HTMLButtonElement[] = $state([]);

  /** Left / Right / Home / End move the focus between the pages; Enter and Space activate. */
  function onkeydown(event: KeyboardEvent): void {
    const index = buttons.findIndex((b) => b === document.activeElement);
    if (index < 0) {
      return;
    }
    let target: number | null = null;
    switch (event.key) {
      case "ArrowRight":
        target = (index + 1) % buttons.length;
        break;
      case "ArrowLeft":
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

<header class="topbar">
  <button type="button" class="brand" aria-label="About Cartload" title="About Cartload" onclick={() => navigation.go("settings", "about")}>
    <span class="glyph"><Cartridge size={24} /></span>
    <span class="wordmark"><Wordmark height={15} /></span>
  </button>
  <nav aria-label="Pages">
    <ul role="list">
      {#each PAGES as page, i (page.id)}
        <li>
          <button
            type="button"
            class:active={page.id === navigation.page}
            aria-current={page.id === navigation.page ? "page" : undefined}
            title={`${page.hint} (Ctrl+${i + 1})`}
            bind:this={buttons[i]}
            onclick={() => navigation.go(page.id)}
            {onkeydown}
          >
            <Icon name={page.icon} size={18} />
            <span>{page.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>
</header>

<style>
  .topbar {
    display: flex;
    align-items: stretch;
    gap: var(--space-5);
    height: 48px;
    padding: 0 var(--space-4);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: var(--space-1) 0;
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
  }

  nav,
  ul {
    display: flex;
    align-items: stretch;
  }

  ul {
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
  }

  li button {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border: 0;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: color var(--motion-fast) var(--ease-out);
  }

  li button:hover {
    color: var(--text);
  }

  li button.active {
    color: var(--accent-strong);
    font-weight: var(--weight-semibold);
  }

  /* The current page is underlined against the bar's bottom border. */
  li button.active::after {
    content: "";
    position: absolute;
    left: var(--space-2);
    right: var(--space-2);
    bottom: -1px;
    height: 3px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  @media (max-width: 599px) {
    .wordmark {
      display: none;
    }
  }
</style>
