/** Fixtures for `pnpm dev` in a plain browser, where no backend exists: `?preview=<state>`
 *  shows one of the states. The wording mirrors `src-tauri/src/status.rs`. */

import type { StatusReport } from "../api/types";

const NOTHING = "Nothing found. Connect the console with USB and turn it on.";

const fixtures: Record<string, () => StatusReport> = {
  disconnected: () => ({
    status: { state: "disconnected", hint: { kind: "nothing" } },
    tone: "neutral",
    title: NOTHING,
    next:
      "A console with hakchi installed shows up a few seconds after power-on. To install hakchi, hold Reset while turning the console on so it starts in FEL mode.",
    details: [],
    changed_at: Date.now(),
  }),
  fel: () => ({
    status: {
      state: "fel",
      device: {
        bus: "1",
        address: 9,
        vendor_id: 0x1f3a,
        product_id: 0xefe8,
        manufacturer: null,
        product: null,
        serial: null,
      },
      check: { kind: "fel", soc_id: 0x00166700, soc: "Allwinner R16", r16: true },
    },
    tone: "ready",
    title: "Console in FEL mode: ready to install the kernel.",
    next: "Open the Kernel page to install hakchi, or to boot it from memory without installing anything.",
    details: [
      { label: "USB", value: "bus 1, device 9" },
      { label: "USB id", value: "1f3a:efe8" },
      { label: "Chip", value: "Allwinner R16" },
    ],
    changed_at: Date.now(),
  }),
  booting: () => ({
    status: { state: "booting", phase: "network", address: null, since: Date.now() - 4000 },
    tone: "busy",
    title: "hakchi kernel found on USB: waiting for its network link…",
    next: "This usually takes a few seconds.",
    details: [{ label: "Waiting for", value: "4 s" }],
    changed_at: Date.now(),
  }),
  connected: () => ({
    status: {
      state: "connected",
      address: "169.254.13.37:22",
      link: { kind: "usb", interface: "hakchi0", host_address: "169.254.68.74" },
      since: Date.now() - 60_000,
      info: {
        hwid: "0461872a872802079d2818c76c118000",
        md_partitioning: false,
        minimal_memboot: false,
        versions: { boot: "1.0.3", kernel: "3.4.113.29-madmonkey", script: "v1.0.4-126" },
        can_interact: true,
        console: "SnesEur",
        system_code: "snes-eur",
        firmware: "Nand",
        game_sync_path: "/var/lib/hakchi/games",
        original_games_path: "/usr/share/games",
        rootfs_path: "/var/lib/hakchi/rootfs",
        squashfs_path: "/var/squashfs",
        config: { usb_host: "y", usb_rw: "n" },
        storage: {
          path: "/var/lib/hakchi/games",
          usage: { total: 369_737_728, used: 32_718_848, free: 325_582_848 },
          saves: 139_264,
          external_saves: false,
          games: { all: 3_145_728, non_multiboot: 0, collections: { "snes-eur": 3_145_728 }, extra: 0 },
        },
        original_games: Array.from({ length: 21 }, (_, i) => `CLV-P-SAA${i}E`),
      },
    },
    tone: "ready",
    title: "SNES Classic Mini, EUR, hakchi v1.0.4-126, 310 MB free",
    next: "Ready. Open the Library page to add and sync games.",
    details: [
      { label: "Model", value: "SNES Classic Mini" },
      { label: "Region", value: "EUR" },
      { label: "Hardware id", value: "0461872a872802079d2818c76c118000" },
      { label: "hakchi", value: "v1.0.4-126" },
      { label: "Kernel", value: "3.4.113.29-madmonkey" },
      { label: "Boot", value: "1.0.3" },
      { label: "Firmware", value: "stock NAND firmware" },
      { label: "Games storage", value: "/var/lib/hakchi/games" },
      { label: "Storage", value: "353 MB total, 31 MB used, 310 MB free" },
      { label: "Games on console", value: "3 MB" },
      { label: "Save states", value: "136 KB" },
      { label: "Original games", value: "21" },
      { label: "Address", value: "169.254.13.37:22" },
      { label: "Link", value: "USB (hakchi0, host 169.254.68.74)" },
    ],
    changed_at: Date.now(),
  }),
  stock: () => ({
    status: {
      state: "fel",
      device: {
        bus: "1",
        address: 9,
        vendor_id: 0x1f3a,
        product_id: 0xefe8,
        manufacturer: "Nintendo",
        product: "CLV-S-SNPY",
        serial: null,
      },
      check: { kind: "not_fel", reason: "the device did not answer within 1s (verify device)" },
    },
    tone: "warning",
    title: "Console attached over USB, but not in FEL mode.",
    next: "It is on stock firmware in normal boot. Switch it off, hold Reset, switch it on while still holding Reset for two seconds, then release Reset.",
    details: [
      { label: "USB", value: "bus 1, device 9" },
      { label: "USB id", value: "1f3a:efe8" },
      { label: "Manufacturer", value: "Nintendo" },
      { label: "Product", value: "CLV-S-SNPY" },
      { label: "Error", value: "the device did not answer within 1s (verify device)" },
    ],
    changed_at: Date.now(),
  }),
};

/** The fixture named by `?preview=`, or the disconnected state. */
export function previewReport(): StatusReport {
  const name =
    typeof location !== "undefined" ? new URLSearchParams(location.search).get("preview") : null;
  const make = (name && fixtures[name]) || fixtures["disconnected"]!;
  return make();
}
