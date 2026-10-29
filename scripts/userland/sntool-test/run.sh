#!/usr/bin/env bash
# Tests our sntool's sunxi-part (hakchi fork, branch cartload: sntool/sunxi_part.c):
#
#   scripts/userland/sntool-test/run.sh [hakchi checkout]
#
# 1. its layout logic, built for this computer, against the fixtures (test_sunxi_part.c);
# 2. with a console backup (BACKUP=<dir from scripts/console-backup.sh>): our sntool and the
#    shipped one, both under qemu-arm against copies of that backup (fakenand/run.sh), must
#    write the same boot area and partition table and run the same commands for grow, shrink
#    and tune. Needs scripts/userland/build.sh's toolchain and the shipped package unpacked
#    in target/analysis/pkg (tar -xzf data/hakchi.hmod). The same for flashing u-boot and the
#    kernel (sunxi-flash burn_boot1, burn_boot2, and hakchi's uninstall's "default").
#
# The checkout defaults to the one scripts/userland/build.sh uses (target/userland/src/hakchi).
set -euo pipefail

repo=$(cd "$(dirname "$0")/../../.." && pwd)
src=$(cd "${1:-$repo/target/userland/src/hakchi}" && pwd)
engine=$(command -v podman || command -v docker)
out=$repo/target/sntool-test
mkdir -p "$out"

echo "== layout logic (host build)"
"$engine" run --rm -v "$repo:/repo:ro,z" -v "$src:/hakchi:ro,z" -v "$out:/out:z" cartload-userland-build \
  sh -c 'gcc -std=gnu99 -Wall -Wextra -Werror -O2 -I/hakchi/sntool -o /out/test_sunxi_part \
    /repo/scripts/userland/sntool-test/test_sunxi_part.c /hakchi/sntool/sunxi_part.c \
    && /out/test_sunxi_part /repo/scripts/userland/fixtures'

[ -n "${BACKUP:-}" ] || { echo "(BACKUP not set: skipping the comparison with the shipped sntool)"; exit 0; }

echo "== our sntool (ARM build)"
"$engine" run --rm -v "$src:/hakchi:z" -v "$repo/target/userland:/work:z" -v "$out:/out:z" cartload-userland-build \
  sh -c 'export PATH=/work/buildroot-2015.11.1/output/host/usr/bin:$PATH; cd /hakchi/sntool \
    && make -s clean >/dev/null 2>&1; make -s GCC=arm-buildroot-linux-gnueabihf-gcc >/out/build.log 2>&1 \
    || { tail -20 /out/build.log; exit 1; }; cp sntool /out/sntool; make -s clean >/dev/null 2>&1; true'

shipped=$repo/target/analysis/pkg/bin/sntool
fake=$repo/scripts/userland/fakenand/run.sh
F=$repo/target/fakenand
# A start with the factory layout in all four copies: copies 1 and 2's boot-info replaced by
# copy 3's (the one the console never wrote), the partition table of a shrunk console.
for who in shipped ours; do
  mkdir -p "$F/factory-$who/root/dev"
  cp "$BACKUP/bootarea.bin" "$F/factory-$who/raw.bin"
  for o in $((0x1ba000)) $((0x29a000)); do
    dd if="$BACKUP/bootarea.bin" bs=4096 skip=$((0x37a000 / 4096)) count=8 status=none \
      | dd of="$F/factory-$who/raw.bin" bs=4096 seek=$((o / 4096)) conv=notrunc status=none
  done
  truncate -s 16M "$F/factory-$who/root/dev/nand"
  dd if="$repo/scripts/userland/fixtures/snes-eur-nand-mbr-shrunk.bin" of="$F/factory-$who/root/dev/nand" \
    conv=notrunc status=none
done
# Each case: name, start (an earlier case, or empty for the backup), environment, command.
cases=(
  "list|||"
  "shrink|||shrink"
  "shrink-dry|||shrink --dry-run"
  "grow-shrunk|shrink||grow"
  "grow-dry|shrink||grow --dry-run"
  "grow-grown|||grow"
  "grow-factory|factory||grow"
  "shrink-factory|factory||shrink"
  "tune|||tune"
  "grow-normal|shrink|FAKENAND_NORMAL=1|grow"
  "shrink-normal||FAKENAND_NORMAL=1|shrink"
  # fsck exit codes from 4 are uncorrected errors. (1 is "errors corrected": the shipped sntool
  # stops there too, ours goes on, the one difference.)
  "shrink-fail-fsck||FAKENAND_FAIL=fsck FAKENAND_FAIL_CODE=4|shrink"
  "shrink-fail-64bit||FAKENAND_FAIL=-fs|shrink"
  "shrink-fail-resize||FAKENAND_FAIL=612352s|shrink"
  "shrink-fail-tune2fs||FAKENAND_FAIL=tune2fs|shrink"
)
fail=0
for c in "${cases[@]}"; do
  IFS='|' read -r name from env args <<<"$c"
  for who in shipped ours; do
    prog=$shipped; [ $who = ours ] && prog=$out/sntool
    from_case=${from:+$from-$who}
    # shellcheck disable=SC2086
    env FAKENAND_FROM=$from_case $env "$fake" "$BACKUP" "$name-$who" "$prog" sunxi-part $args >/dev/null 2>&1 || true
  done
  diffs=""
  cmp -s "$F/$name-shipped/raw.bin" "$F/$name-ours/raw.bin" || diffs+=" boot-area"
  cmp -s "$F/$name-shipped/root/dev/nand" "$F/$name-ours/root/dev/nand" || diffs+=" partition-table"
  cmp -s "$F/$name-shipped/stdout.txt" "$F/$name-ours/stdout.txt" || diffs+=" stdout"
  # The commands started, in order (the reads and the pids differ).
  for who in shipped ours; do
    sed -n 's/^\[[0-9]*\] \(popen\|  running\)/\1/p' "$F/$name-$who/log.txt" >"$F/$name-$who/commands.txt"
  done
  cmp -s "$F/$name-shipped/commands.txt" "$F/$name-ours/commands.txt" || diffs+=" commands"
  if [ -z "$diffs" ]; then
    echo "  $name: same"
  else
    echo "  $name: differs in$diffs (target/fakenand/$name-shipped and -ours)"
    fail=1
  fi
done

# Flashing u-boot and the kernel (sunxi-flash burn_boot1 / burn_boot2), as hakchi's install and
# uninstall do: name, start, command, input. "default" (hakchi's uninstall) puts Nintendo's u-boot
# back in the factory arrangement; the cases after it start from its result.
printf 'default\n' >"$out/default.txt"
uboot=$repo/target/analysis/pkg/boot/uboot.bin
flashes=(
  "flash-uboot||burn_boot1 8|$uboot"
  "flash-kernel||burn_boot2 30|$repo/target/analysis/pkg/boot/boot.img"
  "flash-not-uboot||burn_boot1 8|$repo/target/analysis/pkg/boot/boot.img"
  "flash-default||burn_boot1|$out/default.txt"
  "flash-default-again|flash-default|burn_boot1|$out/default.txt"
)
for c in "${flashes[@]}"; do
  IFS='|' read -r name from args input <<<"$c"
  for who in shipped ours; do
    prog=$shipped; [ $who = ours ] && prog=$out/sntool
    FAKENAND_FROM=${from:+$from-$who} "$fake" "$BACKUP" "$name-$who" "$prog" sunxi-flash nandsize >/dev/null 2>&1 || true
    w=$F/$name-$who
    cp "$input" "$w/root/tmp/input"
    # shellcheck disable=SC2086
    "$engine" run --rm --network none -v "$w:/w:z" -v "$repo/target/userland/buildroot-2015.11.1/output:/br:ro,z" \
      -v "$F:/shim:ro,z" -w /w/root/tmp cartload-fakenand sh -c "qemu-arm -L /w/root -E LD_PRELOAD=/shim/fakenand.so \
      -E FAKENAND_RAW=/w/raw.bin -E FAKENAND_LOG=/w/flash.log /w/root/bin/$(basename "$prog") sunxi-flash $args \
      <input >/w/stdout.txt 2>/w/stderr.txt; echo \$? >/w/exit.txt" || true
  done
  if cmp -s "$F/$name-shipped/raw.bin" "$F/$name-ours/raw.bin" && cmp -s "$F/$name-shipped/exit.txt" "$F/$name-ours/exit.txt"; then
    echo "  $name: same"
  else
    echo "  $name: differs (target/fakenand/$name-shipped and -ours)"
    fail=1
  fi
done
# Reading them back, as hakchi's flashBoot checks a flash and the backup copies the boot area.
for args in "read_boot1" "read_boot1 8" "read_boot2" "read_boot2 30"; do
  for who in shipped ours; do
    prog=$shipped; [ $who = ours ] && prog=$out/sntool
    w=$F/flash-uboot-$who
    "$engine" run --rm --network none -v "$w:/w:z" -v "$repo/target/userland/buildroot-2015.11.1/output:/br:ro,z" \
      -v "$F:/shim:ro,z" -w /w/root/tmp cartload-fakenand sh -c "qemu-arm -L /w/root -E LD_PRELOAD=/shim/fakenand.so \
      -E FAKENAND_RAW=/w/raw.bin -E FAKENAND_LOG=/w/read.log /w/root/bin/$(basename "$prog") sunxi-flash $args 2>/dev/null \
      | md5sum" >"$out/read-$who.txt" || true
  done
  cmp -s "$out/read-shipped.txt" "$out/read-ours.txt" && echo "  ${args// /-}: same" || { echo "  ${args// /-}: differs"; fail=1; }
done
# sunxi-part on that factory arrangement (what hakchi's uninstall runs next).
for c in "list-default|flash-default|" "shrink-default|flash-default|shrink --dry-run" "grow-default|flash-default|grow --dry-run"; do
  IFS='|' read -r name from args <<<"$c"
  for who in shipped ours; do
    prog=$shipped; [ $who = ours ] && prog=$out/sntool
    # shellcheck disable=SC2086
    FAKENAND_FROM=$from-$who "$fake" "$BACKUP" "$name-$who" "$prog" sunxi-part $args >/dev/null 2>&1 || true
  done
  cmp -s "$F/$name-shipped/stdout.txt" "$F/$name-ours/stdout.txt" && cmp -s "$F/$name-shipped/raw.bin" "$F/$name-ours/raw.bin" \
    && echo "  $name: same" || { echo "  $name: differs"; fail=1; }
done

# Where ours is stricter than the shipped sntool (that one runs tune's filesystem commands
# outside the recovery shell and with --dry-run). A case passes when the log has what it must and
# lacks what it must not.
expect() { # expect <name> <start> <environment> <args> <grep -E that must match> <that must not>
  local name=$1 from=$2 env=$3 args=$4 must=$5 mustnot=$6 log
  # shellcheck disable=SC2086
  env FAKENAND_FROM=${from:+$from-ours} $env "$fake" "$BACKUP" "$name-ours" "$out/sntool" sunxi-part $args \
    >/dev/null 2>&1 || true
  log=$F/$name-ours/log.txt
  if grep -qE "$must" "$log" "$F/$name-ours/stderr.txt" && ! grep -qE "$mustnot" "$log"; then
    echo "  $name: as expected"
  else
    echo "  $name: NOT as expected (target/fakenand/$name-ours)"
    fail=1
  fi
}
# A start whose first copy has a damaged boot-info block (one byte off: the checksum fails).
mkdir -p "$F/damaged-ours/root/dev"
cp "$BACKUP/bootarea.bin" "$F/damaged-ours/raw.bin"
printf '\x01' | dd of="$F/damaged-ours/raw.bin" bs=1 seek=$((0x1ba300)) conv=notrunc status=none
truncate -s 16M "$F/damaged-ours/root/dev/nand"
dd if="$BACKUP/nand-mbr.bin" of="$F/damaged-ours/root/dev/nand" conv=notrunc status=none

echo "== ours only"
expect tune-normal "" FAKENAND_NORMAL=1 tune "== exit [1-9]" "popen"
expect tune-dry "" "" "tune --dry-run" "\(not\) resizing" "popen"
expect grow-dry-nowrite shrink "" "grow --dry-run" "== exit 0" "phy_write|popen"
# A write that fails on the first copy: nothing after it (the second copy, the driver reload).
expect write-fails shrink FAKENAND_READONLY=1 grow "refused: FAKENAND_READONLY" \
  "phy_write block 0x14|reload_nand|sunxi-nand-part"
expect damaged-copy damaged "" grow "no valid boot-info" "phy_write|popen"
expect unknown-arg "" "" "grow --file x" "don't know what to do" "phy_read|popen"
exit $fail
