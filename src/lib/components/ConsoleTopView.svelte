<script lang="ts">
  /** A Classic Mini console drawn for the model that is (or was last) detected, in the
   *  console's own colours: from above with the front edge at the bottom, or from the front
   *  for the NES, whose buttons are on the front face. The Reset
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
  interface ResetBox {
    x: number;
    y: number;
    w: number;
    h: number;
    from: "right" | "below";
  }

  const reset = $derived.by((): ResetBox => {
    switch (shape) {
      case "snes-usa":
        return { x: 168, y: 176, w: 70, h: 22, from: "right" };
      case "nes":
        return { x: 102, y: 158, w: 34, h: 18, from: "below" };
      case "famicom":
        return { x: 200, y: 214, w: 24, h: 24, from: "right" };
      case "md":
        return { x: 62, y: 204, w: 28, h: 18, from: "below" };
      default:
        return { x: 194, y: 164, w: 34, h: 48, from: "right" };
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
  viewBox="0 0 340 300"
  role="img"
  aria-label="{caption}{shape === "nes" ? " seen from the front" : " seen from above"}. The Reset control is highlighted: hold it while switching Power on."
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
    <!-- NES Classic Mini, seen from the front, since its buttons are on the front face:
         the light grey top whose cartridge cover forms the top edge, the black band
         running from top to bottom at the right with the two controller ports in it, the
         darker front face cutting in at the sides with the light, Power and Reset at the
         left, two small feet below. -->
    <rect class="nes-top" x="30" y="100" width="280" height="46" rx="3" />
    <!-- the front face cuts in at the sides; two small feet under it -->
    <path class="nes-face" d="M30 146h280l-12 48H42z" />
    <rect x="45" y="194" width="34" height="5" rx="1" fill="#4a4a4e" />
    <rect x="261" y="194" width="34" height="5" rx="1" fill="#4a4a4e" />
    <!-- the cartridge cover is the top edge itself, from the black band to the left -->
    <rect x="64" y="100" width="162" height="40" rx="2" fill="#e3e3e3" stroke="#c4c4c4" />
    <!-- the black band runs from the top edge to the bottom -->
    <rect x="226" y="100" width="52" height="94" rx="2" fill="#1e1e1f" />
    <!-- light, power, reset -->
    <rect x="46" y="164" width="6" height="6" rx="1" fill="#e03a2f" />
    <rect x="60" y="158" width="34" height="18" rx="3" fill="#c8c8cb" stroke="#8a8a8e" />
    <text class="lbl on-light" x="77" y="170" text-anchor="middle">POWER</text>
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="8" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" fill="#c8c8cb" stroke="#8a8a8e" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y={reset.y + 12} text-anchor="middle">RESET</text>
    <!-- controller ports -->
    <rect x="231" y="152" width="20" height="36" rx="3" fill="#111112" stroke="#3a3a3c" />
    <rect x="236" y="160" width="10" height="20" rx="2" fill="none" stroke="#9a9a9c" stroke-width="1.5" />
    <rect x="255" y="152" width="20" height="36" rx="3" fill="#111112" stroke="#3a3a3c" />
    <rect x="260" y="160" width="10" height="20" rx="2" fill="none" stroke="#9a9a9c" stroke-width="1.5" />
  {:else if shape === "famicom"}
    <!-- Famicom Mini: cream body with the two red controllers lying along its edges, the
         dark red cartridge cover across the upper middle, the raised block with the eject
         slider below it, then the red Power slide at the front left and the red Reset
         button at the front right, and the dark red band across the front. -->
    <rect class="fc-body" x="44" y="28" width="212" height="268" rx="10" />
    {#each [108, 120, 132, 144, 156, 168, 180, 192] as x (x)}
      <line x1={x} y1="29" x2={x} y2="54" stroke="#c8bca5" stroke-width="2" stroke-linecap="round" />
    {/each}
    <rect x="36" y="46" width="22" height="150" rx="4" fill="#b8382f" stroke="#8f2a23" />
    <rect x="242" y="46" width="22" height="150" rx="4" fill="#b8382f" stroke="#8f2a23" />
    <rect x="66" y="92" width="168" height="40" rx="3" fill="#a3272a" stroke="#7d1c1f" />
    <!-- eject slider on its raised block -->
    <rect x="118" y="153" width="64" height="76" rx="4" fill="#e6dcc7" stroke="#c8bca5" />
    <rect x="146" y="165" width="8" height="52" rx="3" fill="#d6cdb9" stroke="#b7ab94" />
    <rect x="124" y="201" width="52" height="18" rx="3" fill="#b8382f" stroke="#8f2a23" />
    <text class="lbl on-cream" x="150" y="243" text-anchor="middle">EJECT</text>
    <!-- power slide -->
    <rect x="56" y="210" width="48" height="32" rx="3" fill="#d6cdb9" stroke="#b7ab94" />
    <rect x="76" y="214" width="24" height="24" rx="3" fill="#b8382f" stroke="#8f2a23" />
    <text class="lbl on-cream" x="80" y="254" text-anchor="middle">POWER</text>
    <!-- reset -->
    <rect x="196" y="210" width="48" height="32" rx="3" fill="#d6cdb9" stroke="#b7ab94" />
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="8" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" fill="#b8382f" stroke="#8f2a23" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="254" text-anchor="middle">RESET</text>
    <rect x="58" y="262" width="184" height="22" rx="3" fill="#a3272a" stroke="#7d1c1f" />
  {:else}
    <!-- Mega Drive / Genesis Mini: a black box wider than it is tall. The big ring sits a
         little right of centre, nearly as tall as the body, with the slot above the ring's
         centre line; the vent grooves run off the left edge above the control cluster at
         the bottom left: the volume slider, the Power slide above the Reset button beside it. -->
    <path class="md-body" d="M44 50h252l14 14v166q0 16-16 16H46q-16 0-16-16V64z" />
    {#each [68, 73, 78, 83, 88, 93, 98, 103, 108, 113, 118, 123, 128, 133, 138, 143, 148, 153, 158, 163] as y (y)}
      <line x1="31" y1={y} x2="70" y2={y} stroke="#3e3e40" stroke-width="2.4" />
    {/each}
    <circle cx="200" cy="136" r="82" fill="#222224" stroke="#4e4e50" stroke-width="2" />
    <!-- the dark magenta section of the ring at the bottom, and the smooth patch above it -->
    <path d="M241 207A82 82 0 0 1 159 207L167 193.2A66 66 0 0 0 233 193.2Z" fill="#5a1f3f" stroke="#4e4e50" />
    <circle cx="200" cy="136" r="66" fill="none" stroke="#4e4e50" />
    <rect x="176" y="174" width="48" height="22" rx="2" fill="#2a2a2c" stroke="#4e4e50" />
    <rect x="150" y="104" width="100" height="14" rx="3" fill="#0f0f10" stroke="#3a3a3c" />
    <!-- volume -->
    <rect x="46" y="180" width="8" height="42" rx="4" fill="#0f0f10" stroke="#3a3a3c" />
    <rect x="43" y="198" width="14" height="7" rx="2" fill="#6b6b6d" />
    <text class="lbl on-md" x="50" y="236" text-anchor="middle">VOL</text>
    <!-- power slide -->
    <rect x="62" y="180" width="28" height="18" rx="3" fill="#0f0f10" stroke="#3a3a3c" />
    <rect x="65" y="183" width="10" height="12" rx="2" fill="#6b6b6d" />
    <text class="lbl on-md" x="96" y="193">POWER</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 7} y={reset.y - 7} width={reset.w + 14} height={reset.h + 14} rx="8" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" fill="#4a4a4c" stroke="#6b6b6d" />
    <text class="lbl reset-on-dark" x={reset.x + reset.w + 6} y={reset.y + 13}>RESET</text>
  {/if}

  <!-- "hold" pointer at the Reset control -->
  <path class="arrow" d={pointer.path} />
  <text class="hint" x={pointer.tx} y={pointer.ty} text-anchor="start">hold</text>

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

  .nes-top {
    fill: #d7d7d7;
    stroke: #b3b3b3;
    stroke-width: 1.5;
  }

  .nes-face {
    fill: #8c8c90;
    stroke: #6e6e72;
  }

  .fc-body {
    fill: #efe6d5;
    stroke: #c8bca5;
    stroke-width: 1.5;
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
