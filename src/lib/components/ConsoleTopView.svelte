<script lang="ts">
  /** A Classic Mini console seen straight from above, the front edge at the bottom, drawn
   *  for the model that is (or was last) detected, in the console's own colours. The Reset
   *  control is highlighted. Inline
   *  SVG, no assets. */
  interface Props {
    /** `ConsoleType` as the backend names it: `SnesEur`, `SnesUsa`, `Nes`, `MdEur`… */
    model?: string;
    width?: number;
  }

  let { model = "SnesEur", width = 250 }: Props = $props();

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
        return "Mega Drive Mini (North America)";
      case "MdJpn":
      case "MdEur":
      case "MdAsia":
        return "Mega Drive Mini";
      default:
        return "SNES Classic Mini (Europe)";
    }
  });

  /* The Reset control's box per shape, for the halo, and where the "hold" pointer comes
     from: the right, or below when the right side is taken. */
  const reset = $derived.by(() => {
    switch (shape) {
      case "snes-usa":
        return { x: 168, y: 176, w: 70, h: 22, from: "right" as const };
      case "nes":
        return { x: 96, y: 268, w: 30, h: 14, from: "right" as const };
      case "famicom":
        return { x: 140, y: 170, w: 28, h: 14, from: "below" as const };
      case "md":
        return { x: 212, y: 86, w: 30, h: 16, from: "right" as const };
      default:
        return { x: 194, y: 164, w: 34, h: 48, from: "right" as const };
    }
  });

  const pointer = $derived.by(() => {
    const cx = reset.x + reset.w / 2;
    const cy = reset.y + reset.h / 2;
    if (reset.from === "below") {
      const tip = reset.y + reset.h + 8;
      return { path: `M${cx} ${tip + 40}V${tip}M${cx - 9} ${tip + 9}l9-9 9 9`, tx: cx + 14, ty: tip + 44 };
    }
    const tip = reset.x + reset.w + 8;
    return { path: `M${tip + 44} ${cy}H${tip}M${tip + 9} ${cy - 9}l-9 9 9 9`, tx: tip + 50, ty: cy + 6 };
  });
</script>

<svg
  class="view"
  {width}
  viewBox="0 0 340 320"
  role="img"
  aria-label="{caption} seen from above. The Reset control is highlighted: hold it while switching Power on."
>
  {#if shape === "sfc"}
    <!-- SNES Classic Mini (Europe) / Super Famicom Mini: light grey body, a dark grey
         rounded plate, the wide slot with its centre notch, then Power, Eject and Reset in
         one centred row; the LED on the light front. No marks or lettering: the drawing is
         a guide to the controls, not a reproduction. -->
    <rect class="sfc-body" x="40" y="28" width="220" height="268" rx="20" />
    <line class="vents" x1="72" y1="37" x2="228" y2="37" />
    <rect class="sfc-plate" x="52" y="48" width="196" height="208" rx="14" />
    <!-- cartridge slot, centred, with the end marks and the centre notch -->
    <rect class="sfc-dark" x="72" y="104" width="156" height="24" rx="7" />
    <line class="sfc-tick" x1="86" y1="110" x2="86" y2="122" />
    <line class="sfc-tick" x1="214" y1="110" x2="214" y2="122" />
    <path d="M144 111h12l-6 7z" fill="#8e8f93" />
    <!-- power slide, eject, reset: one centred row -->
    <rect class="sfc-dark" x="72" y="164" width="34" height="48" rx="5" />
    <rect x="76" y="169" width="26" height="18" rx="3" fill="#6b6c70" />
    <text class="lbl on-dark" x="89" y="228" text-anchor="middle">POWER</text>
    <rect x="120" y="164" width="60" height="48" rx="6" fill="#a9aaad" stroke="#8a8b8f" />
    <text class="lbl on-dark" x="150" y="228" text-anchor="middle">EJECT</text>
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="10" />
    <rect class="sfc-dark" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="5" />
    <text class="lbl reset-on-dark" x={reset.x + reset.w / 2} y="228" text-anchor="middle">RESET</text>
    <!-- the LED on the light front -->
    <rect x="214" y="274" width="9" height="4" rx="2" fill="#bdbdbf" stroke="#8a8b8f" />
  {:else if shape === "snes-usa"}
    <!-- SNES Classic (North America): one grey, defined by its contours. The raised
         cartridge block at the back with its slot band, then Power and Reset as purple bars
         that slide towards and away from you, the grey Eject button standing between them,
         and the controller-port blocks running from the bars past the front edge. -->
    <rect class="usa-body" x="40" y="28" width="220" height="252" rx="8" />
    <!-- the controller-port blocks run from the cartridge block, under the bars, to a little past the front edge -->
    <rect class="usa-body" x="62" y="158" width="70" height="138" rx="4" />
    <rect class="usa-body" x="168" y="158" width="70" height="138" rx="4" />
    <!-- raised cartridge block with the slot flap across its middle -->
    <rect class="usa-raised" x="62" y="84" width="176" height="74" rx="4" />
    <line class="usa-line" x1="72" y1="111" x2="228" y2="111" />
    <line class="usa-line" x1="72" y1="131" x2="228" y2="131" />
    <line class="usa-line" x1="84" y1="111" x2="84" y2="131" />
    <line class="usa-line" x1="216" y1="111" x2="216" y2="131" />
    <!-- power bar and its lights -->
    <circle cx="52" cy="183" r="1.8" fill="#8a8a8e" />
    <circle cx="52" cy="191" r="1.8" fill="#8a8a8e" />
    <rect x="62" y="176" width="70" height="22" rx="4" fill="#5a4a9c" stroke="#3f3374" />
    <line x1="68" y1="181" x2="126" y2="181" stroke="#7d6fc0" stroke-width="2" stroke-linecap="round" />
    <text class="lbl on-light" x="97" y="214" text-anchor="middle">POWER</text>
    <!-- eject -->
    <rect x="138" y="170" width="24" height="56" rx="3" fill="#c4c4c7" stroke="#a0a0a6" />
    <text class="lbl on-light" x="150" y="240" text-anchor="middle">EJECT</text>
    <!-- reset bar -->
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="9" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="4" fill="#5a4a9c" stroke="#3f3374" />
    <line x1={reset.x + 6} y1={reset.y + 5} x2={reset.x + reset.w - 6} y2={reset.y + 5} stroke="#7d6fc0" stroke-width="2" stroke-linecap="round" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="214" text-anchor="middle">RESET</text>
  {:else if shape === "nes"}
    <!-- NES Classic Mini: the vent lines at the back, the lower front with the cartridge
         door, Power with its red light and Reset on the front face at the left. -->
    <rect class="nes-body" x="40" y="28" width="220" height="268" rx="8" />
    {#each [50, 58, 66, 74, 82, 90, 98, 106] as y (y)}
      <line class="nes-vent" x1="56" y1={y} x2="112" y2={y} />
      <line class="nes-vent" x1="188" y1={y} x2="244" y2={y} />
    {/each}
    <path class="nes-front" d="M40 150h220v138q0 8-8 8H48q-8 0-8-8z" />
    <rect x="86" y="168" width="128" height="68" rx="4" fill="#5f5f63" stroke="#4a4a4e" />
    <!-- power with the light -->
    <text class="lbl on-light" x="73" y="262" text-anchor="middle">POWER</text>
    <circle cx="50" cy="275" r="3" fill="#e03a2f" />
    <rect x="58" y="268" width="30" height="14" rx="2" fill="#5f5f63" stroke="#4a4a4e" />
    <!-- reset -->
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="262" text-anchor="middle">RESET</text>
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="6" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="2" fill="#5f5f63" stroke="#4a4a4e" />
  {:else if shape === "famicom"}
    <!-- Famicom Mini: cream body, the two red controllers docked at the sides, the slot
         between them with its red eject lever, Power and Reset at the front left, the red
         front band. -->
    <rect class="fc-body" x="40" y="28" width="220" height="268" rx="12" />
    <rect x="48" y="60" width="44" height="150" rx="4" fill="#b8382f" />
    <rect x="54" y="70" width="32" height="130" rx="3" fill="#c94a40" stroke="#8f2a23" />
    <rect x="208" y="60" width="44" height="150" rx="4" fill="#b8382f" />
    <rect x="214" y="70" width="32" height="130" rx="3" fill="#c94a40" stroke="#8f2a23" />
    {#each [60, 70, 80, 90] as cy (cy)}
      <rect x="62" y={cy + 20} width="16" height="7" rx="2" fill="#e9d9b5" />
      <circle cx="224" cy={cy + 24} r="3.5" fill="#e9d9b5" />
      <circle cx="236" cy={cy + 24} r="3.5" fill="#e9d9b5" />
    {/each}
    <rect x="104" y="60" width="92" height="80" rx="4" fill="#e3d9c4" stroke="#c8bca5" />
    <rect x="116" y="86" width="68" height="14" rx="3" fill="#3f3a35" />
    <rect x="186" y="96" width="10" height="36" rx="3" fill="#b8382f" />
    <!-- power slide -->
    <rect x="104" y="170" width="28" height="14" rx="3" fill="#3f3a35" />
    <rect x="107" y="173" width="10" height="8" rx="2" fill="#d6cdb9" />
    <text class="lbl on-cream" x="118" y="163" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="7" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" fill="#b8382f" stroke="#8f2a23" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="163" text-anchor="middle">RESET</text>
    <path class="fc-band" d="M40 238h220v46q0 12-12 12H52q-12 0-12-12z" />
  {:else}
    <!-- Mega Drive / Genesis Mini: black body, the 16-bit ring around the slot, the volume
         slider at the left, Power at the front left, Reset at the right of the ring. -->
    <rect class="md-body" x="40" y="28" width="220" height="268" rx="22" />
    <circle cx="150" cy="150" r="72" fill="#222224" stroke="#4e4e50" stroke-width="2" />
    <circle cx="150" cy="150" r="58" fill="none" stroke="#4e4e50" />
    <rect x="116" y="146" width="68" height="12" rx="3" fill="#0f0f10" stroke="#3a3a3c" />
    <rect x="56" y="120" width="10" height="48" rx="5" fill="#0f0f10" stroke="#3a3a3c" />
    <rect x="54" y="150" width="14" height="8" rx="2" fill="#6b6b6d" />
    <text class="lbl on-md" x="61" y="182" text-anchor="middle">VOL</text>
    <!-- power slide -->
    <rect x="56" y="254" width="40" height="12" rx="6" fill="#0f0f10" stroke="#3a3a3c" />
    <circle cx="66" cy="260" r="4.5" fill="#6b6b6d" />
    <text class="lbl on-md" x="76" y="282" text-anchor="middle">POWER</text>
    <!-- reset -->
    <text class="lbl reset-on-dark" x={reset.x + reset.w / 2} y="76" text-anchor="middle">RESET</text>
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="9" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="5" fill="#4a4a4c" stroke="#6b6b6d" />
  {/if}

  <!-- "hold" pointer at the Reset control -->
  <path class="arrow" d={pointer.path} />
  <text class="hint" x={pointer.tx} y={pointer.ty} text-anchor="start">hold</text>

  <text class="caption model" x="150" y="314" text-anchor="middle">{caption}</text>
</svg>

<style>
  .view {
    display: block;
    max-width: 100%;
    height: auto;
    font-family: var(--font);
  }

  /* bodies, in each console's own colours; a stroke keeps them readable on a dark page */
  .sfc-body {
    fill: #d7d5d1;
    stroke: #a9a7a3;
    stroke-width: 1.5;
  }

  .sfc-plate {
    fill: #8e8f93;
    stroke: #6f7074;
  }


  .sfc-dark {
    fill: #44454a;
    stroke: #2f3034;
  }

  .sfc-tick {
    stroke: #8e8f93;
    stroke-width: 1.5;
  }

  .usa-body {
    fill: #d9d9db;
    stroke: #a9a9ad;
    stroke-width: 1.5;
  }

  .usa-raised {
    fill: #d2d2d5;
    stroke: #a4a4a8;
  }

  .usa-line {
    stroke: #aeaeb2;
    stroke-width: 1.5;
    stroke-linecap: round;
  }

  .nes-body {
    fill: #d4d4d4;
    stroke: #a6a6a6;
    stroke-width: 1.5;
  }

  .nes-vent {
    stroke: #b5b5b5;
    stroke-width: 2;
  }

  .nes-front {
    fill: #c2c2c4;
  }

  .fc-body {
    fill: #efe6d5;
    stroke: #c8bca5;
    stroke-width: 1.5;
  }

  .fc-band {
    fill: #b8382f;
  }

  .md-body {
    fill: #2a2a2c;
    stroke: #6b6b6d;
    stroke-width: 1.5;
  }

  .vents {
    stroke: #b9b7b3;
    stroke-width: 3;
    stroke-dasharray: 2 4;
  }

  /* labels */
  .lbl {
    font-size: 7px;
    font-weight: 600;
    letter-spacing: 0.08em;
  }

  .on-dark {
    fill: #ececee;
  }

  .on-light {
    fill: #4a4a4e;
  }

  .on-cream {
    fill: #6e5f48;
  }

  .on-md {
    fill: #9a9a9c;
  }

  .reset-on-dark {
    fill: #c9c3ff;
  }

  .reset-on-light {
    fill: #5b4fc4;
  }






  /* the highlight and the pointer use the app's accent */
  .halo {
    fill: color-mix(in srgb, #4b3fd0 24%, transparent);
    stroke: #4b3fd0;
    stroke-width: 3.5;
    animation: halo 1.6s ease-in-out infinite;
  }

  .arrow {
    fill: none;
    stroke: #4b3fd0;
    stroke-width: 4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .hint {
    font-size: 17px;
    font-weight: 700;
    fill: var(--accent-strong);
    paint-order: stroke;
    stroke: var(--surface);
    stroke-width: 4px;
    stroke-linejoin: round;
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
      opacity: 0.75;
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
