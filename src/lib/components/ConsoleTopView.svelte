<script lang="ts">
  /** A Classic Mini console seen straight from above, drawn for the model that is (or was
   *  last) detected, with the Reset control highlighted and the USB cable leaving the back
   *  towards this computer. Inline SVG on the design tokens, no assets. */
  interface Props {
    /** `ConsoleType` as the backend names it: `SnesEur`, `SnesUsa`, `Nes`, `MdEur`… */
    model?: string;
    width?: number;
  }

  let { model = "SnesEur", width = 320 }: Props = $props();

  type Shape = "sfc" | "snes-usa" | "nes" | "famicom" | "md";

  const shape = $derived.by((): Shape => {
    switch (model) {
      case "SnesUsa":
        return "snes-usa";
      case "Nes":
        return "nes";
      case "Famicom":
      case "ShonenJump":
        return "famicom";
      case "MdJpn":
      case "MdUsa":
      case "MdEur":
      case "MdAsia":
        return "md";
      default:
        return "sfc";
    }
  });

  const caption = $derived.by((): string => {
    switch (model) {
      case "SnesUsa":
        return "SNES Classic (North America)";
      case "SuperFamicom":
        return "Super Famicom Mini";
      case "Nes":
        return "NES Classic Mini";
      case "Famicom":
        return "Famicom Mini";
      case "ShonenJump":
        return "Famicom Mini (Shonen Jump edition)";
      case "MdUsa":
        return "Genesis Mini";
      case "MdJpn":
      case "MdEur":
      case "MdAsia":
        return "Mega Drive Mini";
      default:
        return "SNES Classic Mini (Europe)";
    }
  });

  /* Where the highlighted Reset sits, per shape, for the halo and the "hold" pointer. */
  const reset = $derived.by(() => {
    switch (shape) {
      case "snes-usa":
        return { x: 104, y: 146, w: 36, h: 14 };
      case "nes":
        return { x: 96, y: 176, w: 30, h: 12 };
      case "famicom":
        return { x: 128, y: 150, w: 28, h: 14 };
      case "md":
        return { x: 220, y: 92, w: 26, h: 16 };
      default:
        return { x: 208, y: 136, w: 30, h: 16 };
    }
  });
</script>

<svg
  class="view"
  {width}
  viewBox="0 0 360 236"
  role="img"
  aria-label="{caption} seen from above. The Reset control is highlighted: hold it while switching Power on, with the USB cable at the back connected to this computer."
>
  <!-- cable from the back of the console to the computer -->
  <path class="cable" d="M200 40V22H296" />
  <rect class="plug" x="296" y="14" width="22" height="16" rx="3" />
  <text class="caption" x="307" y="48" text-anchor="middle">USB</text>
  <text class="caption" x="307" y="60" text-anchor="middle">to this computer</text>

  {#if shape === "sfc"}
    <!-- Super Famicom shape: a body that widens towards the front, a raised cartridge
         plateau with the slot, the eject button at its front left, the grooves on the
         right, Power (slide) and Reset (button) at the front right. -->
    <path class="body" d="M62 40h176q20 0 22 20l8 110q2 20-18 20H50q-20 0-18-20l8-110q2-20 22-20z" />
    <rect class="plateau" x="92" y="54" width="116" height="74" rx="10" />
    <rect class="slot" x="108" y="70" width="84" height="8" rx="2" />
    <rect class="button" x="104" y="104" width="30" height="12" rx="4" />
    <text class="label" x="119" y="126" text-anchor="middle">EJECT</text>
    <path class="groove" d="M226 62q26 44 0 88" />
    <path class="groove" d="M236 56q30 50 0 100" />
    <path class="groove" d="M246 50q34 56 0 112" />
    <rect class="seam" x="64" y="176" width="172" height="5" rx="2.5" />
    <text class="brand" x="80" y="150" text-anchor="start">CLASSIC MINI</text>
    <!-- power -->
    <rect class="track" x="164" y="138" width="34" height="12" rx="6" />
    <circle class="knob" cx="172" cy="144" r="4.5" />
    <text class="label" x="181" y="166" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="10" />
    <rect class="button accent" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="5" />
    <text class="label accent-text" x={reset.x + reset.w / 2} y="166" text-anchor="middle">RESET</text>
  {:else if shape === "snes-usa"}
    <!-- North American SNES: a boxy two-tone body, the slot plateau in the middle, the
         purple eject button at its right, Power and Reset as slide switches at the front
         left. -->
    <rect class="body" x="40" y="40" width="220" height="150" rx="10" />
    <rect class="front" x="40" y="132" width="220" height="58" rx="10" />
    <rect class="front-square" x="40" y="132" width="220" height="12" />
    <rect class="plateau" x="84" y="52" width="132" height="64" rx="6" />
    <rect class="slot" x="100" y="68" width="100" height="8" rx="2" />
    <rect class="button accent-soft" x="222" y="66" width="14" height="30" rx="4" />
    <text class="label" x="229" y="108" text-anchor="middle">EJECT</text>
    <text class="brand" x="150" y="100" text-anchor="middle">SUPER NINTENDO</text>
    <rect class="seam" x="60" y="178" width="180" height="4" rx="2" />
    <!-- power -->
    <rect class="track" x="58" y="146" width="36" height="14" rx="7" />
    <circle class="knob accent-fill" cx="67" cy="153" r="5" />
    <text class="label" x="76" y="174" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="12" />
    <rect class="track accent" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="7" />
    <circle class="knob accent-fill" cx={reset.x + 9} cy={reset.y + 7} r="5" />
    <text class="label accent-text" x={reset.x + reset.w / 2} y="174" text-anchor="middle">RESET</text>
  {:else if shape === "nes"}
    <!-- NES: a flat box with the vent slits at the back left, the lowered front with its
         cartridge flap, and Power and Reset on the front face at the left. -->
    <rect class="body" x="40" y="40" width="220" height="150" rx="8" />
    {#each [58, 66, 74, 82, 90, 98] as y (y)}
      <line class="vent" x1="56" x2="118" y1={y} y2={y} />
    {/each}
    <rect class="front" x="40" y="120" width="220" height="70" rx="8" />
    <rect class="front-square" x="40" y="120" width="220" height="12" />
    <rect class="flap" x="92" y="136" width="116" height="38" rx="3" />
    <text class="brand" x="150" y="158" text-anchor="middle">NES CLASSIC</text>
    <!-- power, with its LED -->
    <circle class="led" cx="54" cy="182" r="2.5" />
    <rect class="button" x="60" y="176" width="30" height="12" rx="3" />
    <text class="label" x="75" y="204" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="8" />
    <rect class="button accent" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" />
    <text class="label accent-text" x={reset.x + reset.w / 2} y="204" text-anchor="middle">RESET</text>
  {:else if shape === "famicom"}
    <!-- Famicom: the controllers docked at each side, the cartridge slot between them,
         the eject lever at its right, Power and Reset at the front left. -->
    <rect class="body" x="40" y="40" width="220" height="150" rx="10" />
    <rect class="dock" x="48" y="54" width="42" height="96" rx="4" />
    <rect class="dock" x="210" y="54" width="42" height="96" rx="4" />
    <rect class="plateau" x="104" y="52" width="92" height="76" rx="4" />
    <rect class="slot" x="116" y="66" width="68" height="8" rx="2" />
    <rect class="button" x="186" y="84" width="10" height="30" rx="3" />
    <text class="label" x="191" y="126" text-anchor="middle">EJECT</text>
    <rect class="front-strip" x="40" y="160" width="220" height="30" rx="10" />
    <rect class="front-strip-square" x="40" y="160" width="220" height="10" />
    <!-- power -->
    <rect class="track" x="84" y="150" width="34" height="12" rx="6" />
    <circle class="knob" cx="92" cy="156" r="4.5" />
    <text class="label" x="101" y="182" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="10" />
    <rect class="button accent" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="4" />
    <text class="label accent-text" x={reset.x + reset.w / 2} y="182" text-anchor="middle">RESET</text>
  {:else}
    <!-- Mega Drive: the 16-bit ring around the cartridge slot, the volume slider at the
         left, Power at the front left and Reset at the right of the ring. -->
    <rect class="body" x="40" y="40" width="220" height="150" rx="16" />
    <circle class="ring" cx="150" cy="106" r="54" />
    <circle class="ring inner" cx="150" cy="106" r="42" />
    <rect class="slot" x="120" y="102" width="60" height="8" rx="2" />
    <text class="brand" x="150" y="80" text-anchor="middle">16-BIT</text>
    <rect class="track vertical" x="56" y="110" width="10" height="42" rx="5" />
    <circle class="knob" cx="61" cy="140" r="4" />
    <text class="label" x="61" y="166" text-anchor="middle">VOL</text>
    <!-- power -->
    <rect class="track" x="60" y="172" width="34" height="10" rx="5" />
    <circle class="knob" cx="68" cy="177" r="3.5" />
    <text class="label" x="77" y="200" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="10" />
    <rect class="button accent" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="5" />
    <text class="label accent-text" x={reset.x + reset.w / 2} y="128" text-anchor="middle">RESET</text>
  {/if}

  <!-- "hold" pointer at the Reset control -->
  <path class="arrow" d="M{reset.x + reset.w + 34} {reset.y + reset.h / 2}h-20M{reset.x + reset.w + 22} {reset.y + reset.h / 2 - 6}l-7 6 7 6" />
  <text class="hint" x={reset.x + reset.w + 40} y={reset.y + reset.h / 2 + 4} text-anchor="start">hold</text>

  <text class="caption model" x="150" y="226" text-anchor="middle">{caption}</text>
</svg>

<style>
  .view {
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

  .front,
  .front-square,
  .plateau,
  .dock,
  .flap {
    fill: var(--surface-hover);
  }

  .plateau,
  .dock,
  .flap {
    stroke: var(--border-strong);
    stroke-width: 1;
  }

  .front-strip,
  .front-strip-square {
    fill: color-mix(in srgb, var(--danger) 28%, var(--surface-raised));
  }

  .slot {
    fill: var(--bg);
    stroke: var(--border-strong);
  }

  .seam,
  .vent {
    fill: var(--border);
    stroke: var(--border);
    stroke-width: 1.5;
  }

  .groove {
    fill: none;
    stroke: var(--border);
    stroke-width: 1.5;
  }

  .ring {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: 2;
  }

  .ring.inner {
    stroke-width: 1;
  }

  .button {
    fill: var(--surface);
    stroke: var(--border-strong);
  }

  .button.accent {
    fill: var(--accent-soft);
    stroke: var(--accent);
  }

  .button.accent-soft {
    fill: var(--accent-soft);
    stroke: var(--accent);
    stroke-opacity: 0.6;
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

  .led {
    fill: var(--danger);
  }

  .halo {
    fill: var(--accent-soft);
    stroke: var(--accent);
    stroke-width: 1.5;
    animation: halo 1.8s ease-in-out infinite;
  }

  .brand {
    font-size: 7.5px;
    letter-spacing: 0.14em;
    fill: var(--text-faint);
  }

  .label {
    font-size: 7.5px;
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
    stroke-linejoin: round;
  }

  .plug {
    fill: var(--surface-hover);
    stroke: var(--border-strong);
  }

  .caption {
    font-size: 8px;
    fill: var(--text-muted);
  }

  .caption.model {
    font-size: 9px;
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
