<script lang="ts">
  /** A Classic Mini console seen straight from above, the front edge at the bottom, drawn
   *  for the model that is (or was last) detected, in the console's own colours. The Reset
   *  control is highlighted and the USB cable leaves the back towards this computer. Inline
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
        return "Genesis Mini";
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
        return { x: 108, y: 204, w: 44, h: 16, from: "right" as const };
      case "nes":
        return { x: 96, y: 268, w: 30, h: 14, from: "right" as const };
      case "famicom":
        return { x: 140, y: 170, w: 28, h: 14, from: "below" as const };
      case "md":
        return { x: 212, y: 86, w: 30, h: 16, from: "right" as const };
      default:
        return { x: 192, y: 156, w: 28, h: 38, from: "right" as const };
    }
  });

  const pointer = $derived.by(() => {
    const cx = reset.x + reset.w / 2;
    const cy = reset.y + reset.h / 2;
    if (reset.from === "below") {
      const tip = reset.y + reset.h + 6;
      return { path: `M${cx} ${tip + 32}V${tip}M${cx - 6} ${tip + 6}l6-6 6 6`, tx: cx + 10, ty: tip + 34 };
    }
    const tip = reset.x + reset.w + 6;
    return { path: `M${tip + 36} ${cy}H${tip}M${tip + 6} ${cy - 6}l-6 6 6 6`, tx: tip + 42, ty: cy + 4 };
  });
</script>

<svg
  class="view"
  {width}
  viewBox="0 0 340 320"
  role="img"
  aria-label="{caption} seen from above. The Reset control is highlighted: hold it while switching Power on, with the USB cable at the back connected to this computer."
>
  <!-- cable from the back of the console to the computer -->
  <path class="cable" d="M236 28V14h46" />
  <rect class="plug" x="282" y="6" width="22" height="16" rx="3" />
  <text class="caption" x="293" y="38" text-anchor="middle">USB</text>
  <text class="caption" x="293" y="50" text-anchor="middle">to this computer</text>

  {#if shape === "sfc"}
    <!-- SNES Classic Mini (Europe) / Super Famicom Mini: light grey body, a dark grey plate
         with the four-colour mark, the slot with its centre notch, then Power, Eject and
         Reset in a row; the Nintendo badge and the LED on the light front. -->
    <rect class="sfc-body" x="40" y="28" width="220" height="268" rx="20" />
    <line class="vents" x1="72" y1="37" x2="228" y2="37" />
    <path class="sfc-plate" d="M68 48h164q14 0 14 14v134l-14 30H68l-14-30V62q0-14 14-14z" />
    <circle class="sfc-badge" cx="222" cy="74" r="12" />
    <circle cx="217" cy="69" r="3.6" fill="#e03a2f" />
    <circle cx="227" cy="69" r="3.6" fill="#f3c02b" />
    <circle cx="217" cy="79" r="3.6" fill="#2fa84f" />
    <circle cx="227" cy="79" r="3.6" fill="#2f68c9" />
    <rect class="sfc-dark" x="96" y="116" width="108" height="16" rx="5" />
    <line class="sfc-tick" x1="106" y1="120" x2="106" y2="128" />
    <line class="sfc-tick" x1="194" y1="120" x2="194" y2="128" />
    <path d="M146 120h8l-4 5z" fill="#8e8f93" />
    <!-- power slide -->
    <rect class="sfc-dark" x="96" y="156" width="28" height="38" rx="4" />
    <rect x="100" y="160" width="20" height="16" rx="3" fill="#6b6c70" />
    <text class="lbl on-dark" x="110" y="206" text-anchor="middle">POWER</text>
    <!-- eject -->
    <rect x="134" y="156" width="48" height="38" rx="5" fill="#a9aaad" stroke="#8a8b8f" />
    <text class="lbl on-dark" x="158" y="206" text-anchor="middle">EJECT</text>
    <!-- reset -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="8" />
    <rect class="sfc-dark" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="4" />
    <text class="lbl reset-on-dark" x={reset.x + reset.w / 2} y="206" text-anchor="middle">RESET</text>
    <!-- front -->
    <rect x="98" y="240" width="40" height="11" rx="5.5" fill="none" stroke="#7a7a7e" />
    <text class="brand-italic" x="118" y="248.5" text-anchor="middle">Nintendo</text>
    <text class="brand-big" x="98" y="267">SUPER NINTENDO</text>
    <text class="brand-tiny" x="98" y="276">ENTERTAINMENT SYSTEM</text>
    <rect x="214" y="258" width="9" height="4" rx="2" fill="#bdbdbf" stroke="#8a8b8f" />
  {:else if shape === "snes-usa"}
    <!-- SNES Classic (North America): a boxy grey body, the raised deck with the slot and the
         purple eject button, Power and Reset as purple slide switches at the front left. -->
    <rect class="usa-body" x="40" y="28" width="220" height="268" rx="10" />
    <line class="vents" x1="72" y1="37" x2="228" y2="37" />
    <rect class="usa-deck" x="52" y="46" width="196" height="144" rx="6" />
    <rect class="usa-dark" x="78" y="92" width="116" height="16" rx="4" />
    <rect x="206" y="84" width="18" height="32" rx="4" fill="#7f74c8" stroke="#5f55a8" />
    <text class="lbl on-light" x="215" y="128" text-anchor="middle">EJECT</text>
    <text class="brand-mid" x="136" y="154" text-anchor="middle">SUPER NINTENDO</text>
    <text class="brand-tiny" x="136" y="163" text-anchor="middle">ENTERTAINMENT SYSTEM</text>
    <!-- power slide -->
    <rect class="usa-dark" x="56" y="204" width="44" height="16" rx="8" />
    <circle cx="66" cy="212" r="6" fill="#7f74c8" />
    <text class="lbl on-light" x="78" y="234" text-anchor="middle">POWER</text>
    <!-- reset slide -->
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="13" />
    <rect class="usa-dark" x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="8" />
    <circle cx={reset.x + 10} cy={reset.y + 8} r="6" fill="#7f74c8" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="234" text-anchor="middle">RESET</text>
    <!-- front band -->
    <path class="usa-band" d="M40 246h220v40q0 10-10 10H50q-10 0-10-10z" />
    <rect x="98" y="262" width="40" height="11" rx="5.5" fill="none" stroke="#7a7a7e" />
    <text class="brand-italic" x="118" y="270.5" text-anchor="middle">Nintendo</text>
    <rect x="214" y="268" width="9" height="4" rx="2" fill="#bdbdbf" stroke="#8a8b8f" />
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
    <text class="brand-italic on-dark" x="150" y="206" text-anchor="middle">Nintendo</text>
    <text class="brand-tiny" x="150" y="254" text-anchor="middle">ENTERTAINMENT SYSTEM</text>
    <!-- power with the light -->
    <text class="lbl on-light" x="73" y="262" text-anchor="middle">POWER</text>
    <circle cx="50" cy="275" r="3" fill="#e03a2f" />
    <rect x="58" y="268" width="30" height="14" rx="2" fill="#5f5f63" stroke="#4a4a4e" />
    <!-- reset -->
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="262" text-anchor="middle">RESET</text>
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="6" />
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
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="7" />
    <rect x={reset.x} y={reset.y} width={reset.w} height={reset.h} rx="3" fill="#b8382f" stroke="#8f2a23" />
    <text class="lbl reset-on-light" x={reset.x + reset.w / 2} y="163" text-anchor="middle">RESET</text>
    <path class="fc-band" d="M40 238h220v46q0 12-12 12H52q-12 0-12-12z" />
    <text class="brand-italic on-dark" x="150" y="268" text-anchor="middle">Family Computer</text>
  {:else}
    <!-- Mega Drive / Genesis Mini: black body, the 16-bit ring around the slot, the volume
         slider at the left, Power at the front left, Reset at the right of the ring. -->
    <rect class="md-body" x="40" y="28" width="220" height="268" rx="22" />
    <circle cx="150" cy="150" r="72" fill="#222224" stroke="#4e4e50" stroke-width="2" />
    <circle cx="150" cy="150" r="58" fill="none" stroke="#4e4e50" />
    <rect x="116" y="146" width="68" height="12" rx="3" fill="#0f0f10" stroke="#3a3a3c" />
    <text class="brand-tiny on-md" x="150" y="112" text-anchor="middle">16-BIT</text>
    <text class="brand-mid on-md" x="150" y="238" text-anchor="middle">{model === "MdUsa" ? "GENESIS" : "MEGA DRIVE"}</text>
    <rect x="56" y="120" width="10" height="48" rx="5" fill="#0f0f10" stroke="#3a3a3c" />
    <rect x="54" y="150" width="14" height="8" rx="2" fill="#6b6b6d" />
    <text class="lbl on-md" x="61" y="182" text-anchor="middle">VOL</text>
    <!-- power slide -->
    <rect x="56" y="254" width="40" height="12" rx="6" fill="#0f0f10" stroke="#3a3a3c" />
    <circle cx="66" cy="260" r="4.5" fill="#6b6b6d" />
    <text class="lbl on-md" x="76" y="282" text-anchor="middle">POWER</text>
    <!-- reset -->
    <text class="lbl reset-on-dark" x={reset.x + reset.w / 2} y="76" text-anchor="middle">RESET</text>
    <rect class="halo" x={reset.x - 5} y={reset.y - 5} width={reset.w + 10} height={reset.h + 10} rx="9" />
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

  .sfc-badge {
    fill: #d7d5d1;
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

  .usa-deck {
    fill: #c6c6ca;
    stroke: #a0a0a6;
  }

  .usa-dark {
    fill: #4a4a50;
    stroke: #35353a;
  }

  .usa-band {
    fill: #e6e6e8;
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

  .brand-italic {
    font-size: 6px;
    font-weight: 700;
    font-style: italic;
    fill: #5a5a5e;
  }

  .brand-italic.on-dark {
    fill: #ececee;
  }

  .brand-big {
    font-size: 9.5px;
    font-weight: 800;
    letter-spacing: 0.02em;
    fill: #4a4a4e;
  }

  .brand-mid {
    font-size: 8px;
    font-weight: 800;
    letter-spacing: 0.04em;
    fill: #6a6a70;
  }

  .brand-tiny {
    font-size: 4.6px;
    font-weight: 600;
    letter-spacing: 0.12em;
    fill: #6a6a70;
  }

  .brand-mid.on-md,
  .brand-tiny.on-md {
    fill: #9a9a9c;
  }

  /* the highlight and the pointer use the app's accent */
  .halo {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    animation: halo 1.8s ease-in-out infinite;
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
      opacity: 0.4;
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
