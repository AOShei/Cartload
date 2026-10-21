<script lang="ts">
  /** The cartridge glyph: logo, status icon and loading indicator. */
  interface Props {
    size?: number;
    /** Pulses while something is in progress. */
    busy?: boolean;
    label?: string;
  }

  let { size = 48, busy = false, label }: Props = $props();
</script>

<svg
  class="cartridge"
  class:busy
  width={size}
  height={size}
  viewBox="0 0 48 48"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  focusable="false"
>
  <path
    class="body"
    d="M12 6h24a2 2 0 0 1 2 2v34H10V8a2 2 0 0 1 2-2z"
    fill="currentColor"
    opacity="0.18"
  />
  <path
    class="outline"
    d="M12 6h24a2 2 0 0 1 2 2v34H10V8a2 2 0 0 1 2-2z"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linejoin="round"
  />
  <path d="M16 6v8h16V6" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
  <rect class="label" x="16" y="20" width="16" height="12" rx="1.5" fill="currentColor" opacity="0.55" />
  <path d="M14 42v-5h20v5" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
</svg>

<style>
  .cartridge {
    display: block;
    flex: none;
  }

  .busy .label {
    animation: pulse 1.6s ease-in-out infinite;
  }

  .busy {
    animation: bob 1.6s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
    }
    50% {
      opacity: 0.85;
    }
  }

  @keyframes bob {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-2px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .busy,
    .busy .label {
      animation: none;
    }
  }
</style>
