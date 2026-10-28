<script lang="ts">
  import { toasts } from "../../state";
  import Icon from "../Icon.svelte";
  import { highlightJson } from "./json";

  /* Read-only JSON in a dark, code-editor-like box with a copy button. The highlighting
     only colours tokens rendered as text; nothing is editable. */
  interface Props {
    /** Pretty-printed JSON. */
    text: string;
    /** What the copy button's label and toast call it. */
    label?: string;
  }

  let { text, label = "the technical details" }: Props = $props();
  const tokens = $derived(highlightJson(text));
  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      if (timer) {
        clearTimeout(timer);
      }
      timer = setTimeout(() => (copied = false), 1500);
    } catch {
      toasts.error(`Could not copy ${label}.`);
    }
  }
</script>

<div class="code">
  <button type="button" class="copy" onclick={() => void copy()} aria-label={`Copy ${label}`} title={copied ? "Copied" : "Copy"}>
    <Icon name={copied ? "check" : "copy"} size={14} />
    <span>{copied ? "Copied" : "Copy"}</span>
  </button>
  <pre>{#each tokens as token, i (i)}<span class={token.kind}>{token.text}</span>{/each}</pre>
</div>

<style>
  /* The same dark box in both themes, like an editor pane. */
  .code {
    --code-bg: #1e1f24;
    --code-text: #d4d6dd;
    --code-key: #8ab4f8;
    --code-string: #a8d58b;
    --code-number: #f0b072;
    --code-literal: #c79bf2;
    --code-punct: #8b8f9a;

    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    color-scheme: dark;
    border: 1px solid #000;
    border-radius: var(--radius-m);
    background: var(--code-bg);
    overflow: hidden;
  }

  pre {
    flex: 1;
    margin: 0;
    padding: var(--space-3) var(--space-4);
    overflow: auto;
    color: var(--code-text);
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-sm);
    line-height: 1.5;
    user-select: text;
  }

  .key {
    color: var(--code-key);
  }

  .string {
    color: var(--code-string);
  }

  .number {
    color: var(--code-number);
  }

  .literal {
    color: var(--code-literal);
  }

  .punct {
    color: var(--code-punct);
  }

  .copy {
    position: absolute;
    top: var(--space-2);
    /* Clear of the vertical scrollbar. */
    right: calc(var(--space-3) + 12px);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border: 1px solid #3a3d45;
    border-radius: var(--radius-s);
    background: #2a2c33;
    color: var(--code-text);
    font: inherit;
    font-size: var(--text-xs, 12px);
    cursor: pointer;
  }

  .copy:hover {
    background: #34373f;
  }

  .copy:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
