/** Fixtures for `pnpm dev` in a plain browser, where no backend exists: `?preview=<state>`
 *  shows one of the states. The wording mirrors `src-tauri/src/status.rs`. */

import type { FlowFailure, FlowSnapshot, PackageStatus, StatusReport, StepSnapshot } from "../api/types";

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

const INSTALL_STEPS: [string, string, string][] = [
  ["memboot", "Boot from memory", "Boot the hakchi kernel from RAM: through FEL, or with kexec from a running hakchi kernel"],
  ["wait-console", "Wait for the console", "Wait for the console to come up on the USB network (169.254.13.37) and accept SSH"],
  ["detect", "Identify the console", "Run the identification sequence and check that the console is in the memboot shell"],
  ["check-md", "Mega Drive check", "Refuse to install over Project Lunar on a Mega Drive Mini"],
  ["grow", "Grow the data partition", "Merge the unused UDISK partition into the data partition (sunxi-part grow)"],
  ["reboot-grown", "Reboot into the grown partition", "Boot the kernel again with kexec so it sees the grown partition"],
  ["wait-console", "Wait for the console", "Wait for the console to come up on the USB network (169.254.13.37) and accept SSH"],
  ["detect", "Identify the console", "Run the identification sequence and check that the console is in the memboot shell"],
  ["prepare", "Prepare the install", "Set the install flags and create the transfer directory"],
  ["upload", "Upload the package", "Upload hakchi.hmod to /hakchi/transfer and check its MD5"],
  ["install", "Run the installer", "Run the device-side installer (boot): back up the stock kernel, flash u-boot and the hakchi kernel, install the rootfs, reboot"],
  ["wait-console", "Wait for the console", "Wait for the console to come up on the USB network (169.254.13.37) and accept SSH"],
  ["verify", "Verify the install", "Reconnect and check the installed versions against the package"],
];

function pendingSteps(): StepSnapshot[] {
  return INSTALL_STEPS.map(([name, label, description], index) => ({
    index,
    name,
    label,
    description,
    state: "pending",
    started_at: null,
    elapsed_ms: null,
    message: null,
    transfer: null,
  }));
}

const FEL_FAILURE: FlowFailure = {
  step_index: 0,
  step_name: "memboot",
  kind: "timeout",
  title: "The console did not show up in FEL mode within 120 s.",
  cause: "It is not in FEL mode, or the USB link is not working.",
  next: "Power the console off, hold Reset, switch Power on, keep Reset held for two seconds and release it; check the USB cable and, on Linux, the udev rule for 1f3a:efe8.",
  detail:
    "step 1 (memboot) failed: a console in FEL mode (USB 1f3a:efe8) appearing did not happen within 120 s; power the console off, hold Reset, switch Power on, keep Reset held for two seconds and release it; check the USB cable and, on Linux, the udev rule for 1f3a:efe8",
  wants_stock_kernel: false,
  wants_force: false,
};

const flowFixtures: Record<string, () => FlowSnapshot> = {
  idle: () => ({
    seq: 0,
    kind: null,
    state: "idle",
    steps: pendingSteps(),
    current: null,
    message: null,
    failure: null,
    cancel_pending: false,
    started_at: null,
    finished_at: null,
  }),
  running: () => {
    const now = Date.now();
    const steps = pendingSteps();
    const finished: [number, StepSnapshot["state"], number, string | null][] = [
      [0, "done", 4200, null],
      [1, "done", 11300, null],
      [2, "done", 900, null],
      [3, "skipped", 300, "not a Mega Drive partition layout"],
      [4, "done", 2600, null],
      [5, "done", 6100, null],
      [6, "done", 14800, null],
      [7, "done", 800, null],
      [8, "done", 400, null],
    ];
    for (const [index, state, elapsed, message] of finished) {
      const step = steps[index]!;
      step.state = state;
      step.elapsed_ms = elapsed;
      step.started_at = now - 60_000;
      step.message = message;
    }
    const upload = steps[9]!;
    upload.state = "running";
    upload.started_at = now - 2_300;
    upload.message = "uploading hakchi.hmod (9340382 bytes, hakchi v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey))";
    upload.transfer = { label: "hakchi.hmod", done: 5_242_880, total: 9_340_382 };
    return {
      seq: 41,
      kind: { kind: "install" },
      state: "running",
      steps,
      current: 9,
      message: upload.message,
      failure: null,
      cancel_pending: false,
      started_at: now - 44_000,
      finished_at: null,
    };
  },
  failed: () => {
    const now = Date.now();
    const steps = pendingSteps();
    const first = steps[0]!;
    first.state = "failed";
    first.started_at = now - 121_000;
    first.elapsed_ms = 120_040;
    first.message = FEL_FAILURE.detail;
    return {
      seq: 7,
      kind: { kind: "install" },
      state: "failed",
      steps,
      current: 0,
      message: FEL_FAILURE.detail,
      failure: FEL_FAILURE,
      cancel_pending: false,
      started_at: now - 121_000,
      finished_at: now - 1_000,
    };
  },
  done: () => {
    const now = Date.now();
    const steps = pendingSteps();
    const timings = [4200, 11300, 900, 300, 2600, 6100, 14800, 800, 400, 3900, 184_000, 42_000, 1_900];
    steps.forEach((step, index) => {
      step.state = index === 3 ? "skipped" : "done";
      step.message = index === 3 ? "not a Mega Drive partition layout" : null;
      step.elapsed_ms = timings[index] ?? 1000;
      step.started_at = now - 300_000;
    });
    return {
      seq: 90,
      kind: { kind: "install" },
      state: "done",
      steps,
      current: null,
      message: "installed: hakchi v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey) installed on SNES Classic Mini (EUR)",
      failure: null,
      cancel_pending: false,
      started_at: now - 300_000,
      finished_at: now - 20_000,
    };
  },
  cancelled: () => {
    const now = Date.now();
    const steps = pendingSteps();
    const first = steps[0]!;
    first.message = "Stopped before it finished.";
    return {
      seq: 3,
      kind: { kind: "memboot" },
      state: "cancelled",
      steps: steps.slice(0, 3),
      current: null,
      message: "Stopped.",
      failure: null,
      cancel_pending: false,
      started_at: now - 9_000,
      finished_at: now - 2_000,
    };
  },
};

/** The flow fixture named by `?flow=`, or none. */
export function previewFlow(): FlowSnapshot | null {
  const name =
    typeof location !== "undefined" ? new URLSearchParams(location.search).get("flow") : null;
  const make = name ? flowFixtures[name] : undefined;
  return make ? make() : flowFixtures["idle"]!();
}

const packageFixtures: Record<string, () => PackageStatus> = {
  cached: () => ({
    package: {
      path: "/home/user/.cache/cartload/hakchi.hmod",
      source: "cached",
      version: "v1.0.4-126",
      boot: "1.0.3",
      kernel: "3.4.113.29-madmonkey",
      len: 9_340_382,
      modified: Date.now() - 3 * 86_400_000,
    },
    cache_path: "/home/user/.cache/cartload/hakchi.hmod",
    sources: [
      "https://github.com/TeamShinkansen/Hakchi2-CE/releases/download/supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod",
      "https://hakchi.net/hakchi/hmods/hakchi-latest.hmod",
    ],
    problem: null,
  }),
  none: () => ({
    package: null,
    cache_path: "/home/user/.cache/cartload/hakchi.hmod",
    sources: ["https://github.com/TeamShinkansen/Hakchi2-CE/releases/download/supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod"],
    problem: null,
  }),
  problem: () => ({
    package: null,
    cache_path: "/home/user/.cache/cartload/hakchi.hmod",
    sources: [],
    problem: "hakchi.hmod: /home/user/.cache/cartload/hakchi.hmod is not a readable gzip tar archive: invalid gzip header",
  }),
};

/** The package fixture named by `?package=`, or the cached one. */
export function previewPackage(): PackageStatus {
  const name =
    typeof location !== "undefined" ? new URLSearchParams(location.search).get("package") : null;
  const make = (name && packageFixtures[name]) || packageFixtures["cached"]!;
  return make();
}
