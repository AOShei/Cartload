#!/usr/bin/env bash
# Copies everything a console keeps on its NAND to this computer, before risky work such as
# installing an experimental kernel.
#
#   scripts/console-backup.sh [DIR]     default: ~/cartload-backups/<date>-<console hwid>
#
# The console must be running hakchi and be connected over USB. The script boots it into
# hakchi's memboot shell from RAM (the pinned kernel package, data/hakchi.hmod), where none of
# its partitions are mounted, so each copy is consistent. Nothing on the console is written.
# It then copies, each checked against an MD5 taken on the console:
#
#   nanda.img nandb.img nandc.img   the partitions (boot, Nintendo's system, data: games, saves,
#                                   hakchi and its settings)
#   boot0.bin boot1.bin             the boot loader and u-boot (hakchi's readBoot0/readBoot1)
#   boot2.img                       the installed kernel (readBoot2)
#   backup2.img                     the stock kernel backup hakchi keeps at block 0x68, if any
#   info.txt                        partition table, versions, command line
#
# and restarts the console into its own system (sync; umount -ar; reboot -f).
#
# Restoring the data partition, from the memboot shell (cartload-flow memboot --hmod
# data/hakchi.hmod): cartload-console exec --stdin --command-timeout 0 \
#   'dd of=/dev/nandc bs=1M && sync' < nandc.img
# then check: cartload-console exec 'md5sum /dev/nandc' against MD5SUMS.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
# Release builds: the debug build's SSH runs at about 0.5 MB/s, the release one at about 4.5 MB/s.
console() { cargo run -q --release -p cartload-console -- exec --command-timeout 0 "$@" 2>/dev/null; }

echo "== console"
cargo build -q --release -p cartload-console -p cartload-flow
hwid=$(console 'hakchi hwid' | tr -dc '0-9a-f')
[ -n "$hwid" ] || { echo "no console answers over USB" >&2; exit 1; }
dir=${1:-$HOME/cartload-backups/$(date +%Y-%m-%d)-$hwid}
mkdir -p "$dir"
echo "console $hwid -> $dir"

echo "== memboot shell (RAM only)"
cargo run -q --release -p cartload-flow -- memboot --hmod data/hakchi.hmod | grep -E '^\[|finished'
console 'grep -q "^/dev/nand" /proc/mounts' \
  && { echo "a NAND partition is mounted in the memboot shell; not copying" >&2; exit 1; }

{
  echo "# Cartload console backup, $(date -R)"
  console 'echo "hwid $(hakchi hwid)"; cat /proc/version; cat /proc/cmdline; echo; cat /proc/partitions; sunxi-part 2>&1'
} >"$dir/info.txt"

copy() { # copy <file> <command printing the data> <command printing its md5>
  printf '%-12s' "$1"
  console "$2" >"$dir/$1"
  local remote local_
  remote=$(console "$3" | awk '{print $1}')
  local_=$(md5sum "$dir/$1" | awk '{print $1}')
  if [ -n "$remote" ] && [ "$remote" = "$local_" ]; then
    printf '%12s bytes  md5 %s ok\n' "$(stat -c %s "$dir/$1")" "$local_"
  else
    echo " md5 mismatch: console $remote, copy $local_" >&2
    exit 1
  fi
}
for p in nanda nandb nandc; do
  copy "$p.img" "dd if=/dev/$p bs=1M 2>/dev/null" "md5sum /dev/$p"
done
copy boot0.bin 'hakchi readBoot0' 'hakchi readBoot0 | md5sum'
copy boot1.bin 'hakchi readBoot1' 'hakchi readBoot1 | md5sum'
copy boot2.img 'hakchi readBoot2' 'hakchi readBoot2 | md5sum'
if console 'hakchi haveBackup2' >/dev/null; then
  copy backup2.img 'hakchi readBackup2' 'hakchi readBackup2 | md5sum'
else
  echo "backup2.img  none (no stock kernel backup at block 0x68 on this console)"
fi
(cd "$dir" && md5sum -- *.img *.bin >MD5SUMS && sha256sum -- *.img *.bin >SHA256SUMS)

echo "== restart into the console's own system"
timeout 20 cargo run -q --release -p cartload-console -- exec 'sync; umount -ar; reboot -f' >/dev/null 2>&1 || true
echo "done: $dir"
