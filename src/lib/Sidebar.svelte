<script lang="ts">
  import { SECTIONS, type Section } from "./sections";

  interface Props {
    current: Section;
    onselect: (section: Section) => void;
  }

  let { current, onselect }: Props = $props();
</script>

<nav class="sidebar" aria-label="Sections">
  <div class="brand">Cartload</div>
  <ul>
    {#each SECTIONS as section (section)}
      <li>
        <button
          type="button"
          class:active={section === current}
          aria-current={section === current ? "page" : undefined}
          onclick={() => onselect(section)}
        >
          {section}
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 12rem;
    padding: 0.75rem;
    background: var(--surface-raised);
    border-right: 1px solid var(--border);
  }

  .brand {
    padding: 0.5rem 0.75rem 1rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  button {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 0;
    border-radius: 0.375rem;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  button:hover {
    background: var(--surface-hover);
  }

  button.active {
    background: var(--accent);
    color: var(--on-accent);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
