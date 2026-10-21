<script lang="ts">
  /** The app icon as inline SVG: the cartridge on the accent tile. The same drawing is the
   *  source of the icon set (`src-tauri/icons/icon.svg`); keep the two in step. */
  interface Props {
    size?: number;
    label?: string;
  }

  let { size = 64, label }: Props = $props();
</script>

<svg
  class="mark"
  width={size}
  height={size}
  viewBox="0 0 256 256"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  focusable="false"
>
  <defs>
    <linearGradient id="cartload-mark-bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#6557de" />
      <stop offset="1" stop-color="#4b3ec6" />
    </linearGradient>
  </defs>
  <rect width="256" height="256" rx="56" fill="url(#cartload-mark-bg)" />
  <path d="M60 44h136a10 10 0 0 1 10 10v124H50V54a10 10 0 0 1 10-10z" fill="#ffffff" />
  <path d="M84 44v24h88V44z" fill="#4b3ec6" opacity="0.35" />
  <rect x="78" y="90" width="100" height="62" rx="8" fill="#5546d4" />
  <path d="M66 178h124v26H66z" fill="#ffffff" opacity="0.72" />
</svg>

<style>
  .mark {
    display: block;
    flex: none;
    border-radius: 22%;
    box-shadow: var(--shadow);
  }
</style>
