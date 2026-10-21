<script lang="ts">
  /** The Cartload wordmark: a monoline geometric lowercase built from the same round strokes
   *  as the cartridge glyph, so it renders identically on every platform without a font. */
  interface Props {
    /** Height in CSS pixels; the width follows. */
    height?: number;
    /** Accessible name; decorative without one (the sidebar brand labels itself). */
    label?: string;
  }

  let { height = 18, label }: Props = $props();
  const width = $derived(Math.round((height * 115) / 22));
</script>

<svg
  class="wordmark"
  {width}
  {height}
  viewBox="0 0 115 22"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  focusable="false"
>
  <g fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
    <path d="M14.64 4.37A8 8 0 1 0 14.64 16.63" />
    <circle cx="25.3" cy="13.5" r="5.2" />
    <path d="M30.5 8.5V18.5" />
    <path d="M37 18.5V8.5M37 12.5A4 4 0 0 1 41 8.5H42.5" />
    <path d="M50.5 5.5V15.5A3 3 0 0 0 53.5 18.5H55M48 8.5H54" />
    <path d="M61.5 2.5V18.5" />
    <circle cx="73.2" cy="13.5" r="5.2" />
    <circle cx="90.1" cy="13.5" r="5.2" />
    <path d="M95.3 8.5V18.5" />
    <circle cx="107" cy="13.5" r="5.2" />
    <path d="M112.2 2.5V18.5" />
  </g>
</svg>

<style>
  .wordmark {
    display: block;
    flex: none;
  }
</style>
