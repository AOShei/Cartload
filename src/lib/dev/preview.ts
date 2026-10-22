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
    supported: true,
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
    supported: true,
    changed_at: Date.now(),
  }),
  booting: () => ({
    status: { state: "booting", phase: "network", address: null, since: Date.now() - 4000 },
    tone: "busy",
    title: "hakchi kernel found on USB: waiting for its network link…",
    next: "This usually takes a few seconds.",
    details: [{ label: "Waiting for", value: "4 s" }],
    supported: true,
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
    supported: true,
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
      { label: "Product", value: "CLV-S-SNPY" },
      { label: "Error", value: "the device did not answer within 1s (verify device)" },
    ],
    supported: true,
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
  ["memboot", "Boot from memory", "Boot the hakchi kernel from RAM, through FEL or kexec"],
  ["wait-console", "Wait for the console", "Wait for the console on the USB network"],
  ["detect", "Identify the console", "Check the console is in the memboot shell"],
  ["check-md", "Compatibility check", "Check that the console is one Cartload supports"],
  ["grow", "Grow the data partition", "Merge the unused UDISK partition into the data partition"],
  ["reboot-grown", "Reboot into the grown partition", "Reboot into the memboot shell with the new partition table"],
  ["wait-console", "Wait for the console", "Wait for the console on the USB network"],
  ["detect", "Identify the console", "Check the console is in the memboot shell"],
  ["prepare", "Prepare the install", "Flag the install and make the transfer directory"],
  ["upload", "Upload the package", "Copy hakchi.hmod to the console"],
  ["install", "Run the installer", "Run the installer: back up, flash, install, reboot"],
  ["wait-console", "Wait for the console", "Wait for the console on the USB network"],
  ["verify", "Verify the install", "Check the installed kernel reports the package's versions"],
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

/* Library page fixtures: `?library=some|empty|many` and `?sync=none|plan|review|running|done|failed`. */

import type {
  GameView,
  LibraryView,
  MenuEntry,
  MenuView,
  SyncPlanView,
  SyncSnapshot,
} from "../api/types";

const PREVIEW_GAMES: [string, "snes" | "nes", string, string, number, boolean][] = [
  ["Chrono Trigger", "snes", "SQUARESOFT", "1995-03-11", 1, false],
  ["ActRaiser", "snes", "ENIX", "1990-12-16", 1, true],
  ["Super Metroid", "snes", "NINTENDO", "1994-03-19", 1, false],
  ["EarthBound", "snes", "NINTENDO", "1994-08-27", 1, true],
  ["Donkey Kong Country", "snes", "NINTENDO", "1994-11-21", 2, false],
  ["Contra III: The Alien Wars", "snes", "KONAMI", "1992-02-28", 2, false],
  ["Mega Man X", "snes", "CAPCOM", "1993-12-17", 1, true],
  ["Kirby's Dream Land 3", "snes", "NINTENDO", "1997-11-27", 2, false],
  ["Castlevania III: Dracula's Curse", "nes", "KONAMI", "1989-12-22", 1, false],
  ["Mega Man 2", "nes", "CAPCOM", "1988-12-24", 1, true],
  ["Metroid", "nes", "NINTENDO", "1986-08-06", 1, false],
  ["Ninja Gaiden", "nes", "TECMO", "1988-12-09", 1, false],
  ["Zelda II: The Adventure of Link", "nes", "NINTENDO", "1987-01-14", 1, true],
  ["Tetris", "nes", "NINTENDO", "1989-11-01", 1, false],
];

function previewCode(index: number, system: "snes" | "nes"): string {
  const letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
  let n = index * 7919 + 13;
  let code = "";
  for (let i = 0; i < 5; i += 1) {
    code += letters[n % 26];
    n = Math.floor(n / 26) + 3;
  }
  return `CLV-${system === "snes" ? "U" : "H"}-${code}`;
}

/** An inline SVG cover, so previews need no files. */
function previewCover(name: string, system: "snes" | "nes", placeholder: boolean): string {
  const hue = (name.length * 37) % 360;
  const bg = placeholder ? "#d8d4cd" : `hsl(${hue} 45% 45%)`;
  const text = placeholder ? "#6b665e" : "#fff";
  const label = name.length > 18 ? `${name.slice(0, 17)}…` : name;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="228" height="204"><rect width="228" height="204" fill="${bg}"/><text x="114" y="104" text-anchor="middle" font-family="sans-serif" font-size="16" fill="${text}">${label.replace(/&/g, "&amp;").replace(/</g, "&lt;")}</text><text x="114" y="130" text-anchor="middle" font-family="sans-serif" font-size="11" fill="${text}" opacity="0.8">${system.toUpperCase()}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

function previewGame(index: number, placeholderEvery: number): GameView {
  const [name, system, publisher, date, players, art] = PREVIEW_GAMES[index % PREVIEW_GAMES.length]!;
  const code = previewCode(index, system);
  const placeholder = !art || index % placeholderEvery === 0;
  const safe = name.replace(/[^A-Za-z0-9]+/g, "_");
  return {
    game: {
      code,
      name: index >= PREVIEW_GAMES.length ? `${name} ${Math.floor(index / PREVIEW_GAMES.length) + 1}` : name,
      sort_name: "",
      system,
      rom_file: system === "snes" ? `${safe}.sfrom` : `${safe}.nes`,
      crc32: 0x2d206bf7 + index,
      file_crc32: 0x2d206bf7 + index,
      original_file: `${name}.${system === "snes" ? "smc" : "nes"}`,
      players,
      simultaneous: players === 2,
      release_date: date,
      publisher,
      copyright: "",
      save_count: 3,
      application: system === "snes" ? "/bin/clover-canoe-shvc-wr -rom" : "/bin/clover-kachikachi-wr",
      args: system === "snes" ? "--volume 100 -rollback-snapshot-period 600" : "--guest-overscan-dimensions 0,0,9,3 --initial-fadein-durations 10,2 --volume 75 --enable-armet",
      tgdb_ids: [1255 + index],
      tgdb_id: placeholder ? null : 1255 + index,
      description: placeholder ? "" : "A classic, still worth a Sunday afternoon.",
      db_name: `${name} (USA).${system === "snes" ? "sfc" : "nes"}`,
      snes: null,
      nes: null,
      placeholder_art: placeholder,
      compress: true,
    },
    size: system === "snes" ? 2_850_539 + index * 1024 : 180_000 + index * 512,
    cover: previewCover(name, system, placeholder),
    thumbnail: previewCover(name, system, placeholder),
    system_name: system === "snes" ? "SNES" : "NES",
  };
}

function lettersMenu(games: GameView[], originals: { code: string; name: string }[]): MenuEntry[] {
  const buckets = new Map<string, MenuEntry[]>();
  for (const view of games) {
    const initial = view.game.name[0]?.toUpperCase() ?? "#";
    const key = /[A-Z]/.test(initial) ? initial : "#";
    const bucket = buckets.get(key) ?? [];
    bucket.push({ kind: "game", code: view.game.code });
    buckets.set(key, bucket);
  }
  const folders: MenuEntry[] = [...buckets.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([letter, entries]) => ({
      kind: "folder",
      name: letter,
      position: "right",
      image: letter === "#" ? "number" : { letter },
      entries: [...entries, { kind: "back", name: "Back", position: "back", image: "back", target: "parent" }],
    }));
  if (originals.length === 0) {
    return folders;
  }
  return [
    ...originals.map((o): MenuEntry => ({ kind: "original", code: o.code })),
    {
      kind: "folder",
      name: "More games...",
      position: "rightmost",
      image: "folder",
      entries: [
        ...folders,
        { kind: "back", name: "Original games", position: "back", image: "back", target: "root" },
      ],
    },
  ];
}

const ORIGINALS = [
  ["CLV-P-SAAAE", "Super Mario World"],
  ["CLV-P-SAABE", "F-ZERO"],
  ["CLV-P-SAAEE", "The Legend of Zelda: A Link to the Past"],
  ["CLV-P-SAAFE", "Super Mario Kart"],
].map(([code, name]) => ({
  code: code!,
  name: name!,
  cover: previewCover(name!, "snes", false),
  thumbnail: previewCover(name!, "snes", false),
}));

/** The library fixture named by `?library=`, or a dozen games. */
export function previewLibrary(): { view: LibraryView; menu: MenuView } {
  const name =
    typeof location !== "undefined" ? new URLSearchParams(location.search).get("library") : null;
  const count = name === "empty" ? 0 : name === "many" ? 500 : PREVIEW_GAMES.length;
  const games = Array.from({ length: count }, (_, i) => previewGame(i, 5)).sort((a, b) =>
    a.game.code.localeCompare(b.game.code),
  );
  const connected =
    typeof location !== "undefined" && new URLSearchParams(location.search).get("preview") === "connected";
  const originals = connected ? ORIGINALS : [];
  const byName = [...games].sort((a, b) => a.game.name.localeCompare(b.game.name));
  return {
    view: {
      root: "/home/user/.local/share/cartload/library",
      games,
      total_size: games.reduce((sum, g) => sum + g.size, 0),
    },
    menu: {
      entries: count === 0 ? [] : lettersMenu(byName, originals),
      options: { structure: { kind: "letters" }, originals_at_root: true },
      folder_art: {},
      custom: name === "many",
      system_code: connected ? "snes-eur" : null,
      originals,
    },
  };
}

function previewPlan(changes: number): SyncPlanView {
  const uploads = Array.from({ length: changes }, (_, i) => ({
    path: `./.storage/CLV-U-AAAA${i}/Game_${i}.sfrom`,
    size: 2_097_152 + i * 4096,
  }));
  return {
    system_code: "snes-eur",
    upload_path: "/var/lib/hakchi/games/snes-eur",
    games: 14,
    originals: 21,
    folders: 6,
    files: 88,
    uploads,
    deletions: changes > 0 ? ["./.storage/CLV-U-ZZZZZ/Old_Game.sfrom", "./.storage/CLV-U-ZZZZZ/CLV-U-ZZZZZ.png"] : [],
    link_deletions: changes > 0 ? ["./000/CLV-P-SAAAE/autoplay"] : [],
    empty_dirs: changes > 0 ? ["./.storage/CLV-U-ZZZZZ"] : [],
    unchanged: 88 - uploads.length,
    total_size: 61_337_600,
    upload_size: uploads.reduce((sum, f) => sum + f.size, 0),
    available: 325_582_848,
    fits: true,
    noop: changes === 0,
    changes: changes === 0 ? 0 : uploads.length + 4,
    storage: { total: 369_737_728, used: 32_718_848, free: 325_582_848 },
    planned_at: Date.now(),
  };
}

/** The sync fixture named by `?sync=`. */
export function previewSync(): { plan: SyncPlanView | null; snapshot: SyncSnapshot | null; reviewing: boolean } {
  const name =
    typeof location !== "undefined" ? new URLSearchParams(location.search).get("sync") : null;
  const now = Date.now();
  const base: SyncSnapshot = {
    seq: 1,
    state: "idle",
    plan: null,
    step: null,
    upload: null,
    log: [],
    report: null,
    error: null,
    started_at: null,
    finished_at: null,
  };
  switch (name) {
    case "plan":
      return { plan: previewPlan(3), snapshot: null, reviewing: false };
    case "review":
      return { plan: previewPlan(3), snapshot: null, reviewing: true };
    case "uptodate":
      return { plan: previewPlan(0), snapshot: null, reviewing: false };
    case "running":
      return {
        plan: previewPlan(3),
        snapshot: {
          ...base,
          seq: 20,
          state: "running",
          plan: previewPlan(3),
          step: { index: 5, total: 9, name: "upload", label: "Upload the games", detail: "3 files, 6.0 MB" },
          upload: {
            path: "./.storage/CLV-U-AAAA1/Game_1.sfrom",
            done: 1_200_000,
            total: 2_101_248,
            overall_done: 3_300_000,
            overall_total: 6_303_744,
          },
          log: ["umount: /var/games: not mounted", "removed 2 files, 1 symlink, 1 directory"],
          started_at: now - 7_000,
        },
        reviewing: false,
      };
    case "done":
      return {
        plan: previewPlan(0),
        snapshot: {
          ...base,
          seq: 40,
          state: "done",
          plan: previewPlan(3),
          step: { index: 9, total: 9, name: "done", label: "Done", detail: "" },
          report: { uploaded_files: 3, uploaded_bytes: 6_303_744, removed_files: 2, removed_links: 1, removed_dirs: 1, games_mounted: true },
          started_at: now - 15_000,
          finished_at: now - 2_000,
        },
        reviewing: false,
      };
    case "failed":
      return {
        plan: previewPlan(3),
        snapshot: {
          ...base,
          seq: 12,
          state: "failed",
          plan: previewPlan(3),
          step: { index: 5, total: 9, name: "upload", label: "Upload the games", detail: "" },
          error: { message: "FTP transfer failed: the console closed the connection.", detail: "ftp: STOR ./.storage/CLV-U-AAAA1/Game_1.sfrom\ncaused by: connection reset by peer" },
          started_at: now - 9_000,
          finished_at: now - 1_000,
        },
        reviewing: false,
      };
    default:
      return { plan: null, snapshot: null, reviewing: false };
  }
}

/** The dialog named by `?dialog=description|advanced|changes`, opened once the page is up. */
export function previewDialog(): string | null {
  return typeof location !== "undefined" ? new URLSearchParams(location.search).get("dialog") : null;
}
