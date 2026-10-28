# Device protocol notes

Verified on an SNES Classic Mini (EUR) from Linux on 2026-10-21 unless stated otherwise.
All three consoles (NES Classic, SNES Classic, Mega Drive Mini) share the Allwinner R16
SoC and the same hakchi tooling; the Mega Drive Mini differs in some paths and is untested here.

## USB identities

| State | USB ID | Notes |
| --- | --- | --- |
| FEL mode (hold Reset while powering on) | `1f3a:efe8` | Allwinner USB boot. Vendor class, bulk endpoints 0x81 in / 0x01 out |
| Stock firmware, normal boot | `1f3a:efe8` | Same ID, with a product string naming the model (below). Nothing answers; do not confuse with FEL |
| Legacy hakchi kernel (pre-2018) | `1f3a:efe8` | "clovershell" protocol. Out of scope: memboot and reinstall instead |
| hakchi kernel or memboot | `04e8:6863`, manufacturer `hakchi` | RNDIS network gadget |

The stock kernel's USB gadget driver carries one product string per model, so a console in
normal boot names itself before anything is installed (seen in the ramdisks of the stock
kernel images; `CLV-S-SNPY` verified on a European SNES Classic Mini):

| Product string | Model |
|---|---|
| `CLV-S-NESY` | NES Classic Mini |
| `CLV-S-HVCY` | Famicom Mini (the Shonen Jump edition too, as far as known) |
| `CLV-S-SHVY` | Super Famicom Mini |
| `CLV-S-SNPY` | SNES Classic Mini, Europe |
| `CLV-S-SNSY` | SNES Classic, North America |

The Shonen Jump edition reports as a Famicom Mini, which it is in a different shell. In FEL
mode no model is reported at all: the boot ROM is the same chip in every console.

Cartload does not support the Mega Drive Mini. It is recognised by its system code only so
that the app can say so when one is connected; no flow or sync starts on it.

The SNES Classic Mini's firmware has hakchi's NES launcher script (`/bin/clover-kachikachi-wr`
and `/usr/bin/clover-kachikachi`) but not the `kachikachi` emulator the script runs for
ordinary mappers (checked on the SNES Classic Mini EUR, 2026-10-22: `/usr/bin/kachikachi`
absent, `canoe-shvc` present). The script's `--retroarch` flag sends the game to RetroArch
(`/bin/nes`, hakchi2 CE's `retroarch` module, `fceumm` core, `nestopia` for FDS). Cartload
adds that flag to NES games synced to a SNES-family console and warns in the plan when
`retroarch-clover` or `/etc/libretro/core/fceumm_libretro.so` is missing.

Emulator modules: Cartload installs RetroArch and cores from KMFD's Mod Hub, the repository
hakchi2 CE ships as its default (`https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/`).
The hub is a directory tree: `<item>.hmod/link` holds the download URL, `<item>.hmod/md5` the
checksum (CE reads the same files, or the whole `pack.tgz`). The catalog Cartload uses is
`_km_retroarch_1910_ozone` (RetroArch 1.9.10 with the `retroarch-clover`, `nes`, `fceumm`,
`nestopia`, `quicknes` launchers), `_km_fceumm_xtreme` (NES core) and `_km_snes9x_xtreme`
(SNES core, with the `snes` and `snes9x` launchers). Install is hakchi2 CE's `ModTasks`:
`mkdir -p /tmp/cartload/hmods/<item>.hmod`, `tar -xzvC` that directory with the `.hmod` (a
gzip tar) on stdin, `hakchi packs_install /tmp/cartload/hmods`, then `reboot -f`; the console
registers each as `<installpath>/hmod/uninstall-<item>`. Verified on hakchi2 CE's own
modules present on the SNES Classic Mini EUR (`retroarch`, `snes9x2010`, ...), which provide
the same launchers under other module names; the KMFD modules themselves are untested on
hardware (see `docs/hardware-tests.md`).

Linux udev: grant user access to `1f3a:efe8` (`MODE="0666", TAG+="uaccess"`) and rename the
`04e8:6863` net device to a fixed name (`hakchi0`) so a NetworkManager keyfile can match it.
Windows needs WinUSB bound to `1f3a:efe8` (Zadig or libwdi). macOS needs nothing.

## Network over USB

- Console address `169.254.13.37/16`, fixed, set by `S92rndis` in the ramdisk. No DHCP server.
- Host must hold a link-local address on the gadget interface (NetworkManager `ipv4.method=link-local`).
- The gadget interface reports `operstate unknown` on Linux (no carrier detection). Any code
  that filters interfaces on "up" must also accept "unknown".
- mDNS: console advertises `_ssh._tcp` via tinysvcmdns, IPv4 only. Treat as secondary; probing
  `169.254.13.37:22` whenever a 169.254 interface exists is the primary discovery path.
  Wi‑Fi consoles (with a USB Wi‑Fi mod) are the only case where mDNS is needed.
- SSH: port 22, user `root`, empty password. The server is dropbear 2018.76 started by inetd as
  `dropbear -iRB`: `-B` grants the blank password through the SSH `none` authentication method
  and the same account is *rejected* through the `password` method, so a client must try `none`
  first (SSH.NET does; russh needs `authenticate_none` before `authenticate_password`). The host
  key (`ecdsa-sha2-nistp256`) is generated in RAM at each boot (`-R`) and cannot be pinned.
  Verified negotiation: `curve25519-sha256`, `aes256-ctr`, `hmac-sha2-256`.
- FTP: port 21, busybox `ftpd -Sw /` from inetd: writable, rooted at `/`, the password is not
  checked. Used for bulk file transfer.
- mDNS announcement (`S93mdnsd`, tinysvcmdns): instance `hakchi`, type `_ssh._tcp`, port 22,
  host `madmonkey.local.`, TXT `hwid=<four hex words>`, `type=<nes|snes|hvcj|md>`,
  `region=<usa|eur|jpn|asia>`.
- firewalld on the host must trust the interface or the console's traffic is dropped.

## FEL memboot (first install and recovery)

Reference: `Fel.cs` and `MembootTasks.cs` in hakchi2 CE. Sequence:

1. Open `1f3a:efe8`, `VerifyDevice()`.
2. Upload `fes1.bin` and execute it to init DRAM (`InitDram`).
3. Upload `uboot.bin` (from the hmod, `boot/uboot.bin`) to the u-boot load address.
4. Upload `boot/boot.img` (kernel + ramdisk) from the hmod to the kernel load address.
   hakchi2 CE patches the boot image's command line in memory for some flows; see `MembootTasks.cs`.
5. Run a u-boot command (`RunUbootCmd`) to boot the image. Console comes up as the RNDIS gadget
   within ~10 s and SSH becomes reachable.

FEL message formats (AWUSBRequest, AWFELMessage, AWFELStandardRequest, AWFELStatusResponse,
AWFELVerifyDeviceResponse) are small fixed-layout structs; the C# files in `Libraries/FelLib/FelLib/`
are the spec. `sunxi-tools` (`sunxi-fel`, C, GPL) is an independent reference for the same protocol.

## Memboot from a running hakchi kernel (kexec)

hakchi2 CE's `MembootTasks.Memboot`, used whenever the console answers SSH and has
`/proc/atags` (an installed hakchi kernel booted normally, or the memboot shell after
`sunxi-part grow`: `GrowTask` with `rebootToRecovery`). CE does not use the `kexec` in the
console's ramdisk or rootfs, nor the device's `hakchi recovery`:

```
uistop                                    <- failure ignored
mkdir -p /tmp/kexec/
cat > /tmp/kexec/kexec && chmod +x ...    <- CE's tools/arm/kexec.static (kexec-tools, static ARM build)
cat > /tmp/kexec/detached-fallback && chmod +x ...   <- CE's tools/arm/detached-fallback
cat > /tmp/kexec/boot.img                 <- the package's boot/boot.img
cd /tmp/kexec/; /bin/sh /tmp/kexec/detached-fallback recovery /tmp/kexec/boot.img   <- 100 ms limit, failure ignored
```

`detached-fallback` re-runs itself in the background (`--detached`), sources `/etc/preinit`,
prepends its own directory to `PATH` (so `kexec` is the uploaded binary), and runs `recovery`,
which calls `reboot_kexec` and then `memboot`: `sntool check` on the image, command line taken
from the image with `hakchi-shell` appended (or `hakchi-clovershell` when clovershell is
active), `kexec -l -t zImage <image> --command-line=... --atags`, then `killall clovershell`,
`service inetd stop`, `uistop`, `sync`, `umount -a -r`, `disableUsbDevice`, `kexec -e`. Without
an image argument it takes `$rootfs/boot/boot.img`, `$installpath/boot/boot.img` or the kernel
partition (`readBoot2`). On a failure before the launch CE runs `uistart` and gives up. The
console drops off USB and comes back as the RNDIS gadget in the memboot shell about 20 s later.

Cartload's first hardware run (2026-10-21, SNES Classic Mini EUR, hakchi v1.0.4-126) ran the
device's own `hakchi recovery` through a detached launcher instead and hung the console (power
LED off, no USB device until a power cycle). Cartload now ships CE's two files (`data/`) and runs
the sequence above under `/tmp/cartload`; that has not been run on hardware yet
(`docs/hardware-tests.md`).

## hakchi.hmod

The kernel package. A gzip tar with: `boot/boot.img`, `boot/uboot.bin`, `bin/` (sntool, kexec,
sunxi-* tools, xdelta3), `etc/preinit.d/`, `install`, `uninstall`, `var/version`.
hakchi2 CE ships v1.0.4-126 from
`https://github.com/TeamShinkansen/Hakchi2-CE/releases/download/supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod`
and also knows `https://hakchi.net/hakchi/hmods/hakchi-latest.hmod` ("Download latest hakchi"
menu) and `https://hakchi.net/hakchi/hakchi.hmod` (its `HakchiUpdateURL` config default). On
2026-10-21 all three answered 200 (GitHub redirects to `release-assets.githubusercontent.com`,
hakchi.net to `static.hakchi.net`) and the first two delivered the same 9 340 382-byte file,
md5 `2e163a25c98b4c05bed274817abb38da`, identical to the copy in the CE build tree. Entry names
inside the tar carry no `./` prefix in that file, but CE accepts both forms and so does Cartload.
`var/version` is a shell fragment: `bootVersion='1.0.3'`, `hakchiVersion='v1.0.4-126'`,
`kernelVersion='3.4.113.29-madmonkey'`. `fes1.bin` ships with the hakchi2 CE source tree
(`hakchi_gui/data/fes1.bin`, 14 048 bytes, starts with the Allwinner `eGON.BT0` header), not in
the hmod; Cartload embeds it from `data/fes1.bin`. Source for the kernel side: madmonkey1907/hakchi (GPL-3).

Cartload pins this one package: SHA-256
`003994b46504959895f40b0b9e91b16951ef4a638feb6312314c8fc4fdbff592`, 9 340 382 bytes
(`cartload_hmod::pinned`). On 2026-10-28 the GitHub asset, `hakchi-latest.hmod` and CE's build
output were still that file. The app carries it (`data/hakchi.hmod`) and writes it to
`<cache dir>/cartload/hakchi.hmod` whenever the cached file is missing or differs; a cached
file with another hash is never used, and the download (a fallback) keeps only a file with the
pinned hash. `CARTLOAD_HMOD` still accepts any valid package, for development.

## Device-side `hakchi` script

The console does the heavy lifting. The app runs `hakchi <subcommand>` over SSH. Subcommands used
by hakchi2 CE, most frequent first: `mount_base`, `umount_base`, `get <var>`, `eval '<shell>'`,
`unset`, `currentFirmware`, `uiresume`, `uipause`, `getBackup2`, `haveBackup2`, `checkExtStorage`,
`pack_install`, `packs_install`, `pack_uninstall`, `pack_list`, `mod_uninstall`, `overmount_games`,
`mmcUsed`, `ismdPartitioning`, `hwid`, `flashBoot2`, `findGameSyncStorage`.
Other commands seen: `uistart`, `sntool check|kernel <img>`.

`/bin/hakchi` dispatches to shell functions from `/etc/preinit.d/b0010_functions` in the ramdisk
(`get` echoes a variable, `eval` runs its argument or, without one, stdin) and ends every
invocation with `save_config`, which rewrites `p0000_config`. Exit codes to know: `uipause` and
`uiresume` end with a `killall` of the Mega Drive UI process and so exit 1 on NES/SNES even when
they worked; `overmount_games` exits 1 when no games are found; `ismdPartitioning` and `mmcUsed`
answer with their exit code only; `hwid` prints four space-separated hex words
(`0461872a 87280207 9d2818c7 6c118000`), which hakchi2 CE joins into one id.

## Connect sequence, identification part (verified)

```
hakchi ismdPartitioning                      -> exit 1 on NES/SNES
source /hakchi/config; [ "$cf_memboot" = "y" ]   -> exit 0 only on a minimal memboot
hakchi hwid
[ -f /var/version ] && echo "yes"
source /var/version && echo "$bootVersion $kernelVersion $hakchiVersion"   -> 1.0.3 3.4.113.29-madmonkey v1.0.4-126
hakchi eval 'echo "$sftype-$sfregion"'       -> snes-eur
hakchi currentFirmware                       -> _nand_ (or the path of a custom firmware)
hakchi get rootfs; hakchi get squashfs       -> /var/lib/hakchi/rootfs, /var/squashfs
cat /etc/preinit.d/p0000_config              -> cfg_xxx='value' lines
```

hakchi2 CE stops after `hwid` on a minimal memboot, and after the versions when the kernel is
older than 3.4.113 / boot 1.0.2 (reflash needed) or the script older than v1.0.4-118 (rootfs
update needed).

## Connect sequence (verified)

```
hakchi findGameSyncStorage          -> path used for games (NAND: /var/lib/hakchi/games)
hakchi get gamepath
df "$(hakchi findGameSyncStorage)" | tail -n 1 | awk '{ print $2 " | " $3 " | " $4 }'
du -d 1 "/var/lib/hakchi/games" | head -n -1 | awk '{ print $2 " | " $1 }'
hakchi eval 'echo "$squashfs$gamepath"'
[ -d "/var/squashfs/usr/share/games" ] && echo YES || echo NO
ls "/var/squashfs/usr/share/games" -p -1 | grep '/'     -> original games (CLV-P-xxxxx dirs)
```
Free space on the SNES Classic after install: about 313 MB on NAND.

## Games model

- Each game is a directory `CLV-U-XXXXX` (user) or `CLV-P-XXXXX` (original) containing the ROM
  (SNES: `.sfrom` container, converted from `.sfc/.smc`; optionally packed as `.sfrom.7z`, see
  "Compressed games"), a `.desktop` launch entry with the command
  line (e.g. `/bin/clover-canoe-shvc-wr -rom /var/games/CLV-U-ZJSQU/Chrono_Trigger.sfrom ...`),
  cover art PNG, and optional `pixelart`, `autoplay` folders.
- Folders are `CLV-S-XXXXX` directories with their own `.desktop` entry and `folder.png`.
  hakchi2 CE defaults to `CLV-S-00000` "Original games" and `CLV-S-00001` "More games...".
- Game metadata (publisher, release date, players) comes from a hash database
  (`romfiles.xml.gz` from TeamShinkansen/hash-db). It has no images.
- Art: TheGamesDB API (needs an API key; CE bundles one) with
  `https://cdn.thegamesdb.net/images/original/clearlogo/{id}.png`. Google image scraping is dead.

## Compressed games (verified on the console on 2026-10-21, read-only)

hakchi2 CE's "Compress games" option packs each ROM file of a game (at most
`NesApplication.MaxCompress` = 16 MiB) into `<file>.7z`: one entry with the ROM's own name,
LZMA (CE's bundled `tiny7z` C# writer, method 1), and points the launch entry's `-rom`
argument at the archive. Nothing else in the entry changes. Chrono Trigger's 4 194 400-byte
`.sfrom` became 2 850 560 bytes in the trace; Cartload's pure-Rust LZMA archive of the same
file is 2 850 539 bytes.

Extraction happens on the console at launch, in the emulator wrappers that hakchi's rootfs
ships for every game (`/bin/clover-canoe-shvc-wr`, `/bin/clover-kachikachi-wr`, `/bin/m2engage`
in `hakchi.hmod`'s ramdisk under `hakchi/rootfs/bin/`): when the ROM path ends in `.7z` the
wrapper runs `tiny7zx x "$filename"` in `$temppath/rom` (`/tmp/rom`, tmpfs), renames the one
extracted file to `rom.<its extension>` and starts the emulator on that. So the entry inside the
archive must keep the ROM's extension, and the archive must hold exactly one file.

`tiny7zx` itself is not part of `hakchi.hmod`. It comes from CE's base mod `tiny7zx.hmod`
(pts-tiny-7z-sfx by Péter Szabó, GPL-2, a 26 578-byte dynamically linked ARM binary, md5
`eb8fa3b5a0d8a832e81eec9b5fccb60b`; decodes LZMA, LZMA2, BCJ, BCJ2 and COPY and keeps the whole
uncompressed file in RAM, which is where the 16 MiB limit comes from). CE transfers all its base
mods (`basehmods.tar`: `clovercon`, `tiny7zx`, `font_remount`, `gamepath-cleanup`, `copyleft`,
`ntfs-3g`) during the kernel install and installs them with the kernel. `font_remount` only
binds an extended title font over the firmware's; it has nothing to do with compression. The
mods are directories in CE's source tree (`hakchi_gui/mods/hmods/<name>.hmod/`). The app
carries `tiny7zx` (`data/tiny7zx.hmod/`) and installs it after every hakchi install; when a sync
has compressed games and `which tiny7zx` fails, the app first runs the base-mods flow (upload,
`hakchi packs_install`, reboot, verify) and then syncs. The `cartload-hmod base-mods` CLI can
still download the mods from CE's tree, pinned to a commit and checked by MD5.

On this console: `which tiny7zx` → `/bin/tiny7zx`; `/var/lib/hakchi/rootfs/bin/tiny7zx` is the
same file; `/var/lib/hakchi/hmod/` holds one `uninstall-<name>` script per installed mod
(`clovercon`, `copyleft`, `font_remount`, `ntfs-3g`, `tiny7zx` plus the user's RetroArch
cores); 256 MB of RAM with about 128 MB free at idle. `/etc/hmodblacklist` lists mod names the
device refuses to install (`tiny7zx-dynamic` among them; `tiny7zx` is not listed).

Installing a mod on a running console (hakchi2 CE `ModTasks`, device script `b0030_package`):

```
mkdir -p '/tmp/hmods/<name>.hmod'
tar -xvC '/tmp/hmods/<name>.hmod'        <- the mod's files as a plain tar on stdin
hakchi packs_install '/tmp/hmods'        -> installing <name>... / package <name> installed
```

`packs_install` runs `pack_install` on every `*.hmod` in the directory: a `.hmod` file is
untarred (`tar -xzf`) to a temporary directory, a directory is used as is; readmes are deleted,
`bin/*` made executable, the mod's `install` script sourced with `$transferpath` set, everything
except `install`, `uninstall` and `transfer` copied into `$rootfs` (`/var/lib/hakchi/rootfs`,
the upper directory of the overlayfs mounted on `/`), and an auto-generated uninstall script
written to `$installpath/hmod/uninstall-<name>`; `hakchi pack_list` prints those names and
`hakchi pack_uninstall <name>` runs the script. CE memboots into recovery, installs with
`mount_base` and then boots normally; Cartload installs on the booted kernel and reboots, so the
files written into the overlay's upper directory are seen by the running system.

Removing a mod (hakchi2 CE `ModTasks.UninstallHmod`): `hakchi pack_uninstall '<name>'` sources
`uninstall-<name>` from `$rootfs` (the generated `rm -f "$rootfs/..."` and
`rmdir --ignore-fail-on-non-empty` lines, after the mod's own uninstall commands), deletes the
record, and prints `uninstalling <name>...` / `package <name> uninstalled`; it exits 0 and
prints nothing when the mod has no record. A mod whose `uninstall` file says `no-uninstall`
gets no record and cannot be removed this way. Cartload reboots afterwards, as for an install
(source: `b0030_package` in the `hakchi.hmod` ramdisk).

The records are named after the package, not the emulator: a SNES Classic set up with hakchi2
CE (checked 2026-10-28) has `retroarch` (RetroArch with the FCEUmm and Nestopia cores, `/bin/nes`),
`snes9x2010` (`/bin/snes`), `genesis_plus_gx` (`/bin/md`) and `mgba` (`/bin/gba`), where
Cartload's own modules are `_km_retroarch_1910_ozone`, `_km_snes9x_xtreme` and so on. Each
record lists the package's files as `rm -f "$rootfs/./bin/<launcher>"`, so Cartload finds the
package behind an emulator with `grep -lE '/bin/(snes9x|snes)"$' uninstall-*` in
`/var/lib/hakchi/hmod`, leaving out RetroArch packages (records with `/bin/retroarch-clover`)
when the emulator is a core.

Cartload pins each module to one file by size and SHA-256 (KMFD's set of 2022-10-31, the hub's
files on 2026-10-28; `crates/hmod/src/modhub.rs`) and mirrors those files as assets of the
Cartload release `emulators-km-2022-10-31`. A download tries the mirror first and the hub's
`link` second, and keeps only a file with the pinned hash; the hub's `md5` file is no longer
read. A newer file on the hub is therefore never picked up until the pins change.

The records carry no version. To tell whether an installed emulator is the build Cartload
pins, Cartload compares files: the cached pinned download against `md5sum` on the console of
the three largest
programs in the package (`bin/`, `usr/bin/`, `*.so`; never fonts, artwork or settings files a
user may edit), at the same paths from `/`. A missing or different file means an update. On the
console checked 2026-10-28, CE's `mgba` has none of `_km_mgba`'s files (CE's core is
`mgba_libretro.so`, the hub's `km_mgba_libretro.so`) and CE's `/bin/retroarch` differs from
`_km_retroarch_1910_ozone`'s, so both show an update. An update uninstalls every record that
provides the emulator (CE's package or Cartload's own older copy), then uploads and installs the
hub's package, and reboots once.

## Sync sequence (verified, see docs/reference/hakchi-ce-sync-trace.log)

1. Build the menu tree locally (folders, resized art, desktop entries).
2. `df`/`du` check of remote storage.
3. `hakchi eval 'umount "$gamepath"'` (exit 1 when nothing was mounted; ignore).
4. Remote diff: delete dirs under `/var/lib/hakchi/games/` that are not system roots, delete
   symlinks under `/var/lib/hakchi/games/snes-eur`, then
   `cd /var/lib/hakchi/games/snes-eur; find . -type f -exec sh -c "stat \"{}\" -c \"%n %s %y\"" \;`
   to list remote files and upload only what changed.
5. Upload over FTP (2.7 MB in 0.4 s on USB).
6. Remote cleanup of empty game dirs.
7. Upload and run `/tmp/linkgamessync.sh` (symlinks games into the menu structure).
8. Sync config.
9. `hakchi overmount_games; uistart` then re-read `df`/`du`.

System roots under the games path are `hvcj-jpn`, `snes-usa|eur|jpn`, `nes-usa|jpn`, `md-jpn|usa|eur|asia`.
