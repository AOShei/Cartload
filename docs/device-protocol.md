# Device protocol notes

Verified on an SNES Classic Mini (EUR) from Linux on 2026-10-21 unless stated otherwise.
All three consoles (NES Classic, SNES Classic, Mega Drive Mini) share the Allwinner R16
SoC and the same hakchi tooling; the Mega Drive Mini differs in some paths and is untested here.

## USB identities

| State | USB ID | Notes |
| --- | --- | --- |
| FEL mode (hold Reset while powering on) | `1f3a:efe8` | Allwinner USB boot. Vendor class, bulk endpoints 0x81 in / 0x01 out |
| Stock firmware, normal boot | `1f3a:efe8` | Same ID, "Nintendo CLV-S-SNPY". Nothing answers; do not confuse with FEL |
| Legacy hakchi kernel (pre-2018) | `1f3a:efe8` | "clovershell" protocol. Out of scope: memboot and reinstall instead |
| hakchi kernel or memboot | `04e8:6863`, manufacturer `hakchi` | RNDIS network gadget |

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
- SSH: port 22, user `root`, empty password. FTP: port 21, same credentials, used for bulk file transfer.
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

## hakchi.hmod

The kernel package. A gzip tar with: `boot/boot.img`, `boot/uboot.bin`, `bin/` (sntool, kexec,
sunxi-* tools, xdelta3), `etc/preinit.d/`, `install`, `uninstall`, `var/version`.
hakchi2 CE ships v1.0.4-126 from
`https://github.com/TeamShinkansen/Hakchi2-CE/releases/download/supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod`
and also knows `https://hakchi.net/hakchi/hmods/hakchi-latest.hmod`. `fes1.bin` ships inside the
app (`data/fes1.bin`), not in the hmod. Source for the kernel side: madmonkey1907/hakchi (GPL-3).

## Device-side `hakchi` script

The console does the heavy lifting. The app runs `hakchi <subcommand>` over SSH. Subcommands used
by hakchi2 CE, most frequent first: `mount_base`, `umount_base`, `get <var>`, `eval '<shell>'`,
`unset`, `currentFirmware`, `uiresume`, `uipause`, `getBackup2`, `haveBackup2`, `checkExtStorage`,
`pack_install`, `packs_install`, `pack_uninstall`, `pack_list`, `mod_uninstall`, `overmount_games`,
`mmcUsed`, `ismdPartitioning`, `hwid`, `flashBoot2`, `findGameSyncStorage`.
Other commands seen: `uistart`, `sntool check|kernel <img>`.

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
  (SNES: `.sfrom` container, converted from `.sfc/.smc`), a `.desktop` launch entry with the command
  line (e.g. `/bin/clover-canoe-shvc-wr -rom /var/games/CLV-U-ZJSQU/Chrono_Trigger.sfrom ...`),
  cover art PNG, and optional `pixelart`, `autoplay` folders.
- Folders are `CLV-S-XXXXX` directories with their own `.desktop` entry and `folder.png`.
  hakchi2 CE defaults to `CLV-S-00000` "Original games" and `CLV-S-00001` "More games...".
- Game metadata (publisher, release date, players) comes from a hash database
  (`romfiles.xml.gz` from TeamShinkansen/hash-db). It has no images.
- Art: TheGamesDB API (needs an API key; CE bundles one) with
  `https://cdn.thegamesdb.net/images/original/clearlogo/{id}.png`. Google image scraping is dead.

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
