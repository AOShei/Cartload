# Help test Cartload

Cartload has been run end to end on one console: a SNES Classic Mini (EUR), on Linux. Every
other console, every other computer and a few features have never touched real hardware.
If you have one of them, a test report is the most useful thing you can contribute.

## How to report

1. Install the latest release from the
   [releases page](https://github.com/AOShei/Cartload/releases). Releases marked
   *pre-release* are the ones that need testing.
2. Try one of the items below.
3. Open **Settings → Activity log → Copy diagnostics**, then
   [open a test report](https://github.com/AOShei/Cartload/issues/new?template=test-report.yml)
   and paste it in. The report never contains your TheGamesDB key. It does contain the
   console's serial number; remove that line if you prefer.

Say so when something worked; that is how items leave this list.

## What needs testing

`docs/hardware-tests.md` has the step-by-step checklist for each item.

| Item | What you need | Risk |
|---|---|---|
| **Kernel install on a never-modified console** | Any supported console on stock firmware | Some: it writes the console's boot flash. The stock kernel is backed up first, and the install resumes if interrupted. Keep the backup the app offers. |
| **NES Classic Mini, Famicom Mini, Shonen Jump edition** | One of them, any state | Low |
| **SNES games on an NES or Famicom Mini** | One of them with hakchi, a SNES controller | None: a game that does not start is the failure mode |
| **macOS** | A Mac (Apple silicon or Intel) and any console | Low |
| **Windows 10 / 11** | A Windows PC and any console; the FEL driver step uses [Zadig](https://zadig.akeo.ie/) | Low |
| **Emulator modules** (RetroArch and cores from KMFD's Mod Hub) | A console with hakchi and no RetroArch yet | Low: modules live in hakchi's storage and a factory reset removes them |
| **Mega Drive / Genesis, Game Boy, Game Boy Advance games** | A console with hakchi, the matching ROMs | None |
| **N64 games** | A console with hakchi, the N64 core | None, but expect most games to run poorly: the console's chip is too slow for most of the library |
| **Pulling the cable during a sync** | Any console with hakchi | Low: the next sync repairs it |

## What has been tested

On a SNES Classic Mini (EUR) under Linux: FEL memboot, the kernel install (from stock and
over a running hakchi kernel), console detection, cover art, and syncing the library from
the app, including NES games through RetroArch.
