#!/usr/bin/env bash
# Runs a console NAND tool (the shipped sntool, or ours) under qemu-arm against copies of a
# console backup instead of a console, and reports what it would change (fakenand.c).
#
#   scripts/userland/fakenand/run.sh <backup dir> <case name> <ARM program> [args...]
#   scripts/userland/fakenand/run.sh ~/cartload-backups/2026-10-28-... grow-dry \
#     target/analysis/pkg/bin/sntool sunxi-part grow --dry-run
#
# <backup dir> is one made by scripts/console-backup.sh (bootarea.bin, nand-mbr.bin). The run
# happens in target/fakenand/<case name>: root/ is the console as qemu sees it (/dev/nand* are
# files: nand starts with the backup's partition table, nanda-c are empty and sized like the
# backup's; raw.bin is a copy of the boot area behind the NAND ioctls), log.txt lists what the
# program opened, the ioctls, the programs it started (they are not run), and after the run the
# blocks and files that changed, and the Linux partition table left in /dev/nand.
# FAKENAND_FROM=<earlier case> starts from that case's boot area and partition table instead of
# the backup's; FAKENAND_NORMAL=1 leaves out the ramdisk's nand.ko (the console's normal system
# instead of hakchi's recovery shell); FAKENAND_READONLY=1 refuses the raw writes;
# FAKENAND_FAIL=<text> makes the started commands containing <text> fail (exit code
# FAKENAND_FAIL_CODE, default 1). Needs scripts/userland/build.sh's
# toolchain and its sunxi-nand-part (target/userland).
set -euo pipefail

[ $# -ge 3 ] || { sed -n '2,21p' "$0" >&2; exit 2; }
backup=$(cd "$1" && pwd)
name=$2
prog=$(realpath "$3")
shift 3

repo=$(cd "$(dirname "$0")/../../.." && pwd)
br=$repo/target/userland/buildroot-2015.11.1/output
[ -x "$br/host/usr/bin/arm-buildroot-linux-gnueabihf-gcc" ] \
  || { echo "no toolchain: run scripts/userland/build.sh first" >&2; exit 1; }
engine=$(command -v podman || command -v docker)
work=$repo/target/fakenand/$name
rm -rf "$work"
mkdir -p "$work/root/dev" "$work/root/proc/1" "$work/root/tmp" "$work/root/bin"

# The shim, built with the console toolchain.
# (Buildroot's toolchain only runs from where it was built: /work.)
"$engine" run --rm -v "$repo:/repo:ro,z" -v "$repo/target/userland:/work:z" \
  -v "$repo/target/fakenand:/out:z" cartload-userland-build \
  /work/buildroot-2015.11.1/output/host/usr/bin/arm-buildroot-linux-gnueabihf-gcc \
  -shared -fPIC -O2 -Wall -o /out/fakenand.so /repo/scripts/userland/fakenand/fakenand.c -ldl

# The console as the program sees it.
if [ -n "${FAKENAND_FROM:-}" ]; then
  # Continue from an earlier case: its boot area and partition table as that run left them.
  cp "$repo/target/fakenand/$FAKENAND_FROM/raw.bin" "$work/raw.bin"
  cp "$repo/target/fakenand/$FAKENAND_FROM/root/dev/nand" "$work/root/dev/nand"
else
  cp "$backup/bootarea.bin" "$work/raw.bin"
  truncate -s 16M "$work/root/dev/nand"
  dd if="$backup/nand-mbr.bin" of="$work/root/dev/nand" conv=notrunc status=none
fi
for p in nanda nandb nandc; do
  truncate -s "$(stat -c %s "$backup/$p.img")" "$work/root/dev/$p"
done
printf 'init\0' >"$work/root/proc/1/cmdline"
printf 'root=/dev/nandb decrypt ro console=ttyS0,115200 loglevel=4\n' >"$work/root/proc/cmdline"
cp "$prog" "$work/root/bin/$(basename "$prog")"
cp "$repo/target/userland/out/bin/sunxi-nand-part" "$work/root/bin/"
# lib: the toolchain's libraries, plus the NAND driver module where hakchi's ramdisk has it (the
# shipped sntool reloads it and does nothing without it; here an empty file, insmod is not run).
mkdir -p "$work/root/lib"
for f in "$br"/target/lib/*; do ln -s "/br/target/lib/$(basename "$f")" "$work/root/lib/"; done
if [ -z "${FAKENAND_NORMAL:-}" ]; then
  mkdir -p "$work/root/lib/modules-ramfs/kernel/drivers/block/nand"
  : >"$work/root/lib/modules-ramfs/kernel/drivers/block/nand/nand.ko"
fi
ln -s /br/target/usr "$work/root/usr"
(cd "$work" && cp -a root before)

image=cartload-fakenand
"$engine" image exists "$image" 2>/dev/null || printf 'FROM docker.io/library/ubuntu:24.04\nRUN apt-get -qq update && DEBIAN_FRONTEND=noninteractive apt-get -qq install -y qemu-user >/dev/null\n' \
  | "$engine" build -q -t "$image" - >/dev/null

set +e
"$engine" run --rm --network none -v "$work:/w:z" -v "$br:/br:ro,z" -v "$repo/target/fakenand:/shim:ro,z" \
  -w /w/root/tmp "$image" \
  qemu-arm -L /w/root -E LD_PRELOAD=/shim/fakenand.so -E FAKENAND_RAW=/w/raw.bin \
  -E FAKENAND_LOG=/w/log.txt -E FAKENAND_ROOT=/w/root -E FAKENAND_NANDPART=/w/root/bin/sunxi-nand-part ${FAKENAND_READONLY:+-E FAKENAND_READONLY=1} ${FAKENAND_FAIL:+-E "FAKENAND_FAIL=$FAKENAND_FAIL"} \
  ${FAKENAND_FAIL_CODE:+-E "FAKENAND_FAIL_CODE=$FAKENAND_FAIL_CODE"} \
  "/w/root/bin/$(basename "$prog")" "$@" >"$work/stdout.txt" 2>"$work/stderr.txt"
status=$?
set -e

{
  echo "== exit $status"
  echo "== changed raw blocks (0x20000 bytes)"
  cmp -l "$backup/bootarea.bin" "$work/raw.bin" 2>/dev/null \
    | awk '{b=int(($1-1)/131072); if (!(b in s)) {s[b]=1; printf "block 0x%02x first change at 0x%x\n", b, $1-1}}'
  echo "== Linux partition table"
  # (sunxi-nand-part opens the device read-write even to list it.)
  "$engine" run --rm --network none -v "$work:/w:z" -v "$br:/br:ro,z" "$image" \
    qemu-arm -L /w/root /w/root/bin/sunxi-nand-part -f a20 /dev/nand 2>&1 | grep -E '^partition|^check' | sort -u || true
  echo "== changed files"
  diff -rq "$work/before" "$work/root" 2>/dev/null | grep -v '^Only in .*: usr$' || true
} >>"$work/log.txt"
cat "$work/stdout.txt" "$work/stderr.txt"
echo "--- $work/log.txt"
cat "$work/log.txt"
