#!/usr/bin/env bash
# Assembles Cartload's console system from source builds: the boot ramdisk, boot.img and the
# package (.hmod), laid out like hakchi v1.0.4-126's, with every file taken from the place
# scripts/userland/manifest.txt names.
#
#   scripts/userland/assemble.sh [hakchi checkout]
#
# Needs scripts/kernel/build.sh's and scripts/userland/build.sh's output (target/kernel,
# target/userland). The hakchi checkout (scripts and data) defaults to the one build.sh uses.
# Into target/assemble/out:
#
#   ramdisk/            the ramdisk's files
#   boot.img            our kernel with that ramdisk
#   cartload.hmod       the package
#   report.txt          every file of the package and ramdisk, where it comes from, its SHA-256,
#                       and the files still kept as shipped
#
# The shipped package (data/hakchi.hmod) gives the layout only: its directories and links, the
# list of files. A shipped program without a manifest line stops the script, as does a program
# needing a library the ramdisk lacks, or a boot.img larger than hakchi's u-boot loads.
set -euo pipefail

BOOT_IMAGE_MAX_LEN=$((0x26 * 0x20000)) # crates/hmod BOOT_IMAGE_MAX_LEN

if [ "${1:-}" = "--inside" ]; then
  # ---- inside the container: /repo the checkout, /work target/userland, /kernel target/kernel,
  # /hakchi the hakchi checkout, /out target/assemble ----
  br=/work/buildroot-2015.11.1/output
  readelf=$br/host/usr/bin/arm-buildroot-linux-gnueabihf-readelf
  manifest=/repo/scripts/userland/manifest.txt
  src=/kernel/src/kernel
  release=$(cat "$src/include/config/kernel.release")
  epoch=$(git -C /hakchi log -1 --format=%ct)
  out=/out/out
  rm -rf /out/shipped "$out"
  mkdir -p /out/shipped/pkg /out/shipped/rd "$out"

  echo "== the shipped layout (data/hakchi.hmod)"
  tar -xzf /repo/data/hakchi.hmod -C /out/shipped/pkg
  /kernel/host/unpackbootimg -i /out/shipped/pkg/boot/boot.img -o /out/shipped >/dev/null
  u=/out/shipped/boot.img
  (cd /out/shipped/rd && xz -dc "$u-ramdisk.gz" | cpio -idm --quiet --no-absolute-filenames)

  isElf() { [ "$(head -c 4 "$1" 2>/dev/null)" = $'\x7fELF' ]; }
  # lookup <rd|pkg> <path>: the manifest's source for it, or nothing
  lookup() { awk -v w="$1" -v p="$2" '$1 == w && $2 == p { print $3; exit }' "$manifest"; }
  report=$out/report.txt
  : >"$report.tmp"
  # place <rd|pkg> <tree> <path> <source>: puts the file there
  place() {
    local where=$1 tree=$2 path=$3 source=$4 dest=$2/$3
    rm -rf "$dest"
    mkdir -p "$(dirname "$dest")"
    case "$source" in
      out:*) cp -L "/work/out/${source#out:}" "$dest" ;;
      br:*) cp -L "$br/target/${source#br:}" "$dest" ;;
      hakchi:*) cp -L "/hakchi/${source#hakchi:}" "$dest" ;;
      shipped) cp -a "/out/shipped/$where/$path" "$dest" ;;
      kernel:ramfs) mkdir -p "$dest" && cp -a "/kernel/ramfs/lib/modules/$release/." "$dest/" ;;
      kernel:modules) mkdir -p "$dest" && cp -a "/kernel/stage/lib/modules/$release" "$dest/" ;;
      drop) return 0 ;;
      boot.img | version) return 0 ;; # made below
      *) echo "manifest: unknown source $source for $where $path" >&2; exit 1 ;;
    esac
    echo "$where $path $source" >>"$report.tmp"
  }

  # build <rd|pkg> <shipped tree> <new tree> <paths the hakchi checkout fills in>
  build() {
    local where=$1 from=$2 to=$3
    shift 3
    local skip=("$@") path source s
    mkdir -p "$to"
    # Directories and links as shipped.
    (cd "$from" && find . -mindepth 1 \( -type d -o -type l \) | sort) | while read -r path; do
      path=${path#./}
      for s in ${skip[@]+"${skip[@]}"}; do case "$path" in "$s" | "$s"/*) continue 2 ;; esac; done
      if [ -L "$from/$path" ]; then
        [ -n "$(lookup "$where" "$path")" ] || ln -s "$(readlink "$from/$path")" "$to/$path"
      else
        mkdir -p "$to/$path"
      fi
    done
    # Files: each from its manifest source; a program without one stops here.
    (cd "$from" && find . -type f | sort) | while read -r path; do
      path=${path#./}
      for s in ${skip[@]+"${skip[@]}"}; do case "$path" in "$s" | "$s"/*) continue 2 ;; esac; done
      source=$(lookup "$where" "$path")
      # A path under a directory the manifest names as a whole (modules).
      [ -n "$source" ] || source=$(awk -v w="$where" -v p="$path" \
        '$1 == w && index(p, $2 "/") == 1 { print "covered"; exit }' "$manifest")
      if [ -z "$source" ]; then
        if isElf "$from/$path"; then
          echo "$where $path: a shipped program with no line in manifest.txt" >&2
          exit 1
        fi
        echo "$where $path: a shipped file with no line in manifest.txt" >&2
        exit 1
      fi
      [ "$source" = covered ] || place "$where" "$to" "$path" "$source"
    done
    # Manifest lines for paths the shipped tree does not have as files (new files, links
    # replaced, whole directories).
    awk -v w="$where" '$1 == w { print $2, $3 }' "$manifest" | while read -r path source; do
      [ -f "$from/$path" ] && [ ! -L "$from/$path" ] && continue
      place "$where" "$to" "$path" "$source"
    done
  }

  echo "== ramdisk"
  rd=$out/ramdisk
  build rd /out/shipped/rd "$rd" init hakchi
  cp -a /hakchi/mod/init "$rd/init"
  cp -a /hakchi/mod/hakchi "$rd/hakchi"
  # Directories git does not keep because they are empty (etc/dropbear: dropbear's host keys).
  (cd /out/shipped/rd && find hakchi -type d -empty) | while read -r d; do
    [ -d "$rd/$d" ] || { mkdir -p "$rd/$d"; echo "rd $d shipped" >>"$report.tmp"; }
  done
  echo "rd init hakchi:mod/init" >>"$report.tmp"
  (cd /hakchi/mod/hakchi && find . \( -type f -o -type l \) | sort) | while read -r path; do
    echo "rd hakchi/${path#./} hakchi:mod/hakchi/${path#./}" >>"$report.tmp"
  done
  place rd "$rd" hakchi/shell/clovershell "$(lookup rd hakchi/shell/clovershell)"
  # Every program finds its libraries in the ramdisk.
  find "$rd" -type f | while read -r f; do
    isElf "$f" || continue
    "$readelf" -d "$f" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p' | while read -r lib; do
      [ -e "$rd/lib32/$lib" ] || { echo "ramdisk ${f#$rd/} needs $lib, which the ramdisk lacks" >&2; exit 1; }
    done
  done
  # The archive: owners root, every time the hakchi checkout's commit time, entries in order,
  # with the kernel's archiver (cpio would record the build's inode numbers); xz with CRC32,
  # as the kernel's decompressor needs.
  find "$rd" -exec touch -h -d "@$epoch" {} +
  sh "$src/scripts/gen_initramfs_list.sh" -u squash -g squash "$rd" >/out/ramdisk.list
  "$src/usr/gen_init_cpio" -t "$epoch" /out/ramdisk.list | xz --check=crc32 -9e >/out/ramdisk.cpio.xz

  echo "== boot.img"
  /kernel/host/mkbootimg --kernel /kernel/out/zImage --ramdisk /out/ramdisk.cpio.xz \
    --cmdline "$(cat "$u-cmdline")" --base "0x$(cat "$u-base")" --kernel_offset "0x$(cat "$u-kerneloff")" \
    --ramdisk_offset "0x$(cat "$u-ramdiskoff")" --second_offset "0x$(cat "$u-secondoff")" \
    --tags_offset "0x$(cat "$u-tagsoff")" --pagesize "$(cat "$u-pagesize")" --hash "$(cat "$u-hash")" \
    -o "$out/boot.img"
  size=$(stat -c %s "$out/boot.img")
  [ "$size" -le "$BOOT_IMAGE_MAX_LEN" ] \
    || { echo "boot.img is $size bytes; hakchi's u-boot loads $BOOT_IMAGE_MAX_LEN" >&2; exit 1; }

  echo "== package"
  pkg=/out/pkg
  rm -rf "$pkg"
  build pkg /out/shipped/pkg "$pkg"
  rm -rf "$pkg/lib/modules/"*-madmonkey
  cp "$out/boot.img" "$pkg/boot/boot.img"
  echo "pkg boot/boot.img boot.img" >>"$report.tmp"
  sed "s/^kernelVersion=.*/kernelVersion='$release'/" /out/shipped/pkg/var/version >"$pkg/var/version"
  echo "pkg var/version version" >>"$report.tmp"
  (cd "$pkg" && tar --owner=0 --group=0 --numeric-owner --sort=name --mtime="@$epoch" -cf - * \
    | gzip -n -9 >"$out/cartload.hmod")

  echo "== report"
  {
    echo "# Cartload console system: kernel $release, hakchi checkout $(git -C /hakchi rev-parse --short HEAD)"
    echo "# boot.img $size bytes (u-boot loads $BOOT_IMAGE_MAX_LEN), ramdisk $(stat -c %s /out/ramdisk.cpio.xz) bytes xz"
    echo "# <rd|pkg> <path> <source> <sha256>"
    sort -u "$report.tmp" | while read -r where path source; do
      f=$rd/$path
      [ "$where" = pkg ] && f=$pkg/$path
      if [ "$source" = shipped ] && [ -d "$f" ]; then
        echo "$where $path $source (an empty directory)"
      elif [ -L "$f" ]; then
        echo "$where $path $source -> $(readlink "$f")"
      elif [ -d "$f" ]; then
        echo "$where $path/ $source ($(find "$f" -type f | wc -l) files)"
      else
        echo "$where $path $source $(sha256sum "$f" | cut -c1-64)"
      fi
    done
    echo
    echo "# Kept as shipped, not yet built by Cartload:"
    awk '$3 == "shipped" { print "#   " $1 " " $2 }' "$manifest"
    echo "# Shipped programs not carried over:"
    awk '$3 == "drop" { print "#   " $1 " " $2 }' "$manifest"
  } >"$report"
  rm -f "$report.tmp"
  ls -l "$out"
  exit 0
fi

# ---- on the host ----
repo=$(cd "$(dirname "$0")/../.." && pwd)
hakchi=$(cd "${1:-$repo/target/userland/src/hakchi}" && pwd)
engine=$(command -v podman || command -v docker || true)
[ -n "$engine" ] || { echo "needs podman or docker" >&2; exit 1; }
[ -f "$repo/target/kernel/out/zImage" ] || { echo "no kernel: run scripts/kernel/build.sh first" >&2; exit 1; }
[ -x "$repo/target/userland/out/bin/sntool" ] || { echo "no userland: run scripts/userland/build.sh first" >&2; exit 1; }
mkdir -p "$repo/target/assemble"
"$engine" run --rm -e TZ=UTC -v "$repo:/repo:ro,z" -v "$repo/target/userland:/work:ro,z" \
  -v "$repo/target/kernel:/kernel:ro,z" -v "$hakchi:/hakchi:ro,z" -v "$repo/target/assemble:/out:z" \
  cartload-userland-build /repo/scripts/userland/assemble.sh --inside
