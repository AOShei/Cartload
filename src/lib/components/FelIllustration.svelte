<script lang="ts">
  /** Front view of a Classic Mini console with the Reset switch highlighted: hold it while
   *  switching Power on, with the USB cable connected to this computer. Inline SVG, no assets. */
  interface Props {
    width?: number;
  }

  let { width = 320 }: Props = $props();
</script>

<svg
  class="fel"
  {width}
  viewBox="0 0 344 180"
  role="img"
  aria-label="Console front view. The Reset switch is highlighted: hold it while switching Power on, with the USB cable connected to this computer."
>
  <!-- cable to the computer -->
  <path class="cable" d="M240 108h24c12 0 12 22 24 22h24" />
  <rect class="plug" x="292" y="122" width="22" height="16" rx="3" />
  <text class="caption" x="302" y="156" text-anchor="middle">USB</text>
  <text class="caption" x="302" y="168" text-anchor="middle">to this computer</text>

  <!-- console body -->
  <rect class="body" x="40" y="56" width="200" height="96" rx="12" />
  <rect class="band" x="40" y="56" width="200" height="30" rx="12" />
  <rect class="band-fill" x="40" y="72" width="200" height="14" />
  <rect class="port" x="72" y="120" width="34" height="18" rx="3" />
  <rect class="port" x="174" y="120" width="34" height="18" rx="3" />
  <text class="brand" x="140" y="110" text-anchor="middle">CLASSIC MINI</text>

  <!-- power switch -->
  <rect class="track" x="58" y="64" width="36" height="14" rx="7" />
  <circle class="knob" cx="66" cy="71" r="5" />
  <text class="label" x="76" y="94" text-anchor="middle">POWER</text>

  <!-- reset switch, highlighted -->
  <rect class="halo" x="104" y="60" width="44" height="22" rx="11" />
  <rect class="track accent" x="108" y="64" width="36" height="14" rx="7" />
  <circle class="knob accent-fill" cx="136" cy="71" r="5" />
  <text class="label accent-text" x="126" y="94" text-anchor="middle">RESET</text>
  <path class="arrow" d="M126 26v22M119 41l7 7 7-7" />
  <text class="hint" x="126" y="18" text-anchor="middle">hold</text>
</svg>

<style>
  .fel {
    display: block;
    max-width: 100%;
    height: auto;
    font-family: var(--font);
    color: var(--text-muted);
  }

  .body {
    fill: var(--surface-raised);
    stroke: var(--border-strong);
    stroke-width: 1.5;
  }

  .band {
    fill: var(--surface-hover);
    stroke: var(--border-strong);
    stroke-width: 1.5;
  }

  .band-fill {
    fill: var(--surface-hover);
  }

  .port {
    fill: var(--surface);
    stroke: var(--border-strong);
  }

  .brand {
    font-size: 8px;
    letter-spacing: 0.14em;
    fill: var(--text-faint);
  }

  .track {
    fill: var(--surface);
    stroke: var(--border-strong);
  }

  .track.accent {
    stroke: var(--accent);
  }

  .knob {
    fill: var(--border-strong);
  }

  .accent-fill {
    fill: var(--accent);
  }

  .halo {
    fill: var(--accent-soft);
    stroke: var(--accent);
    stroke-width: 1.5;
    animation: halo 1.8s ease-in-out infinite;
  }

  .label {
    font-size: 8px;
    font-weight: 600;
    letter-spacing: 0.08em;
    fill: var(--text-muted);
  }

  .accent-text {
    fill: var(--accent-strong);
  }

  .arrow {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .hint {
    font-size: 9px;
    font-weight: 600;
    fill: var(--accent-strong);
  }

  .cable {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: 2.5;
    stroke-linecap: round;
  }

  .plug {
    fill: var(--surface-hover);
    stroke: var(--border-strong);
  }

  .caption {
    font-size: 8px;
    fill: var(--text-muted);
  }

  @keyframes halo {
    0%,
    100% {
      opacity: 0.45;
    }
    50% {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .halo {
      animation: none;
    }
  }
</style>
