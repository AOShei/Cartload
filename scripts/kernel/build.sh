#!/usr/bin/env bash
# Builds Cartload's experimental hakchi kernel and a test package for memboot.
#
#   scripts/kernel/build.sh            build into target/kernel/out
#   scripts/kernel/build.sh --clean    remove target/kernel first
#
# Needs podman or docker and network access for the first run (the Ubuntu image and the two
# git repositories below). Everything else runs inside the container (Containerfile).
#
# Sources, pinned:
#   kernel  AOShei/sun-nontendocm-kernel, branch cartload: the public hakchi kernel with
#           sun_nontendocm_cartload_defconfig (the configuration of hakchi v1.0.4-126's kernel),
#           three Realtek Wi-Fi drivers and the version 3.4.113.30-cartload
#   hakchi  AOShei/hakchi master with its mkbootimg and sunxi-tools submodules, for sntool (the
#           kernel's standby code needs it) and mkbootimg / unpackbootimg
#
# The ramdisk, u-boot and everything else come from the pinned kernel package data/hakchi.hmod.
# The boot image is packed with the parameters read back from that package's boot.img, and the
# script first checks that repacking the package's own kernel and ramdisk gives its boot.img
# byte for byte.
#
# Output (target/kernel/out):
#   zImage, System.map, .config     the kernel
#   boot.img                        the kernel with the package's ramdisk; an appended ramdisk
#                                   archive puts this kernel's NAND and SCSI drivers in
#                                   /lib/modules-ramfs, as the package's ramdisk has madmonkey's
#   hakchi-<release>.hmod           the package with that boot.img and this kernel's modules,
#                                   its installer accepting this kernel
# and in target/kernel/stage/lib/modules/<release> the modules for the console's data partition.
#
# The package boots from RAM (cartload-flow memboot, cartload-fel memboot --shell boot) and
# installs like the pinned one (cartload-flow install --hmod, or the app with CARTLOAD_HMOD).
# Experimental: see docs/kernel.md before installing it.
set -euo pipefail

KERNEL_REPO=https://github.com/AOShei/sun-nontendocm-kernel.git
KERNEL_COMMIT=1ed43a0f2219c78a6d35c7a20f75a83929bec22a
HAKCHI_REPO=https://github.com/AOShei/hakchi.git
HAKCHI_COMMIT=fd525ac710988d12fb9e37213b5fc75651ba47c0
IMAGE=cartload-kernel-build

if [ "${1:-}" = "--inside" ]; then
  # ---- inside the container: /repo is the Cartload checkout, /work is target/kernel ----
  export ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf-
  jobs=$(nproc)
  hakchi=/work/src/hakchi
  kernel=/work/src/kernel
  out=/work/out
  rm -rf /work/host /work/pkg /work/unpacked /work/stage /work/overlay "$out"
  mkdir -p /work/host "$out"

  echo "== host tools: sntool, mkbootimg, unpackbootimg"
  (cd "$hakchi/sntool" && gcc -std=gnu99 -O2 -I. -I../3rdparty/sunxi-tools -I../3rdparty/mkbootimg \
    -DBUILD_COMMIT="\"${HAKCHI_COMMIT:0:7}\"" -DBUILD_DATE="\"cartload\"" \
    crc32.c sha.c sntool.c -o /work/host/sntool)
  env -u ARCH -u CROSS_COMPILE make -s -C "$hakchi/3rdparty/mkbootimg" >/dev/null
  cp "$hakchi/3rdparty/mkbootimg/mkbootimg" "$hakchi/3rdparty/mkbootimg/unpackbootimg" /work/host/
  export PATH="/work/host:$PATH"

  echo "== the package's boot.img"
  mkdir -p /work/pkg /work/unpacked
  tar -xzf /repo/data/hakchi.hmod -C /work/pkg
  unpackbootimg -i /work/pkg/boot/boot.img -o /work/unpacked >/dev/null
  u=/work/unpacked/boot.img
  pack() { # pack <zImage> <ramdisk> <output>
    mkbootimg --kernel "$1" --ramdisk "$2" --cmdline "$(cat $u-cmdline)" \
      --base "0x$(cat $u-base)" --kernel_offset "0x$(cat $u-kerneloff)" \
      --ramdisk_offset "0x$(cat $u-ramdiskoff)" --second_offset "0x$(cat $u-secondoff)" \
      --tags_offset "0x$(cat $u-tagsoff)" --pagesize "$(cat $u-pagesize)" --hash "$(cat $u-hash)" \
      -o "$3"
  }
  pack "$u-zImage" "$u-ramdisk.gz" /work/repack.img
  cmp -s /work/repack.img /work/pkg/boot/boot.img \
    || { echo "repacking the package's own boot.img does not reproduce it" >&2; exit 1; }
  rm /work/repack.img

  echo "== kernel"
  cd "$kernel"
  # The version banner records who built the kernel, where and when; fix all three so that a
  # build of the same commit gives the same zImage.
  export KBUILD_BUILD_USER=cartload KBUILD_BUILD_HOST=cartload-build KBUILD_BUILD_VERSION=1
  export KBUILD_BUILD_TIMESTAMP="$KERNEL_DATE"
  # mkcompile_h keeps an existing compile.h when only the date differs.
  rm -f include/generated/compile.h
  # Vendor drivers print __DATE__ and __TIME__; GCC 4.9 predates SOURCE_DATE_EPOCH, so both
  # are pinned to the commit time by a header forced into every compile (kernel and modules).
  printf '#define __DATE__ "%s"\n#define __TIME__ "%s"\n' "$KERNEL_CDATE" "$KERNEL_CTIME" \
    >/work/reproducible.h
  export KCFLAGS="-Wno-builtin-macro-redefined -include /work/reproducible.h"
  make -s sun_nontendocm_cartload_defconfig
  make -j"$jobs" zImage modules >/work/kernel.log 2>&1 \
    || { tail -40 /work/kernel.log >&2; exit 1; }
  release=$(make -s kernelrelease)
  make -s INSTALL_MOD_PATH=/work/stage INSTALL_MOD_STRIP=1 modules_install >/dev/null

  echo "== out-of-tree modules: mali (r4p0-00rel0), clovercon"
  mali=modules/mali/DX910-SW-99002-r4p0-00rel0/driver/src/devicedrv/mali
  # Mali's Kbuild records BUILD_DATE=$(shell date); a date that prints the commit time keeps
  # the module reproducible.
  mkdir -p /work/mali-bin
  printf '#!/bin/sh\n[ $# -eq 0 ] && { echo "%s"; exit 0; }\nexec /bin/date "$@"\n' "$KERNEL_DATE" \
    >/work/mali-bin/date
  chmod +x /work/mali-bin/date
  PATH="/work/mali-bin:$PATH" make -C "$mali" USING_MMU=1 USING_UMP=0 USING_PMM=1 BUILD=release KDIR="$kernel" \
    >/work/mali.log 2>&1 || { tail -40 /work/mali.log >&2; exit 1; }
  make -C "$kernel" M="$kernel/clovercon" CFLAGS_MODULE=-fno-pic modules \
    >/work/clovercon.log 2>&1 || { tail -40 /work/clovercon.log >&2; exit 1; }
  mods=/work/stage/lib/modules/$release
  mkdir -p "$mods/extra"
  cp "$mali/mali.ko" clovercon/clovercon.ko "$mods/extra/"
  arm-linux-gnueabihf-strip --strip-unneeded "$mods/extra/"*.ko
  rm -f "$mods/build" "$mods/source"
  depmod -b /work/stage -F System.map "$release"
  cp arch/arm/boot/zImage System.map .config "$out/"

  echo "== boot.img"
  # The ramdisk carries what the package's does: the NAND driver, which hakchi's init loads
  # before anything is mounted, and the SCSI disk driver, with what they need. Everything else
  # goes in /lib/modules/<release> on the console. With all modules in the ramdisk (4.8 MB) the
  # console does not start from u-boot at all (no LED), though kexec boots it.
  ramfs=/work/ramfs/lib/modules/$release
  mkdir -p "$ramfs"
  for m in kernel/drivers/block/nand/nand.ko kernel/drivers/scsi/sd_mod.ko; do
    for f in "$m" $(sed -n "s|^$m: *||p" "$mods/modules.dep"); do
      mkdir -p "$ramfs/$(dirname "$f")" && cp "$mods/$f" "$ramfs/$f"
    done
  done
  cp "$mods/modules.order" "$mods/modules.builtin" "$ramfs/"
  depmod -b /work/ramfs "$release"
  mkdir -p /work/overlay/lib32/modules-ramfs
  cp -a "$ramfs/." /work/overlay/lib32/modules-ramfs/
  # The kernel unpacks concatenated archives in order, so this one overrides the package's
  # modules-ramfs while the package's ramdisk keeps its device nodes and owners. The package's
  # ramdisk is xz with CRC32 checks, as the kernel's decompressor needs; so is this one.
  # The kernel's own archiver numbers inodes in order and takes a fixed time, so the archive
  # is the same on every build (cpio would record the build's inode numbers).
  find /work/overlay -exec touch -h -d "@$KERNEL_EPOCH" {} +
  # Sorted by path: find's order depends on the file system.
  sh scripts/gen_initramfs_list.sh -u squash -g squash /work/overlay | LC_ALL=C sort -k2,2 >/work/overlay.list
  usr/gen_init_cpio -t "$KERNEL_EPOCH" /work/overlay.list | xz --check=crc32 -9e >/work/overlay.xz
  ramdisk=/work/ramdisk
  cp "$u-ramdisk.gz" "$ramdisk"
  size=$(stat -c %s "$ramdisk")
  head -c $(( (4 - size % 4) % 4 )) /dev/zero >>"$ramdisk"
  cat /work/overlay.xz >>"$ramdisk"
  pack "$out/zImage" "$ramdisk" "$out/boot.img"

  echo "== package"
  cp "$out/boot.img" /work/pkg/boot/boot.img
  # The package's installer only runs on a kernel named madmonkey ("if no kexec, abort"); this
  # one has kexec too.
  sed -i 's/^(uname -r | grep -qF madmonkey) || return 1$/(uname -r | grep -qE "madmonkey|cartload") || return 1/' \
    /work/pkg/install
  grep -qF 'madmonkey|cartload' /work/pkg/install \
    || { echo "the package's install script no longer has the expected kernel check" >&2; exit 1; }
  rm -rf /work/pkg/lib/modules/*
  cp -a "$mods" /work/pkg/lib/modules/
  sed -i "s/^kernelVersion=.*/kernelVersion='$release'/" /work/pkg/var/version
  (cd /work/pkg && tar --owner=0 --group=0 --numeric-owner --sort=name --mtime="@$KERNEL_EPOCH" -cf - * \
    | gzip -n -9 >"$out/hakchi-$release.hmod")

  echo "== ramdisk modules:" $(cd "$ramfs" && find . -name '*.ko' | sort)
  echo "== done: $release"
  ls -l "$out"
  exit 0
fi

# ---- on the host ----
repo=$(cd "$(dirname "$0")/../.." && pwd)
work=$repo/target/kernel
[ "${1:-}" = "--clean" ] && rm -rf "$work"
engine=$(command -v podman || command -v docker || true)
[ -n "$engine" ] || { echo "needs podman or docker" >&2; exit 1; }

checkout() { # checkout <repo> <commit> <dir>
  [ -d "$3/.git" ] || git clone -q "$1" "$3"
  git -C "$3" fetch -q origin "$2" 2>/dev/null || git -C "$3" fetch -q origin
  git -C "$3" checkout -q --force "$2"
}
mkdir -p "$work/src"
echo "== sources"
checkout "$KERNEL_REPO" "$KERNEL_COMMIT" "$work/src/kernel"
checkout "$HAKCHI_REPO" "$HAKCHI_COMMIT" "$work/src/hakchi"
git -C "$work/src/hakchi" submodule update -q --init 3rdparty/mkbootimg 3rdparty/sunxi-tools

echo "== build image"
"$engine" build -q -t "$IMAGE" -f "$repo/scripts/kernel/Containerfile" "$repo/scripts/kernel" >/dev/null
kernel_epoch=$(git -C "$work/src/kernel" log -1 --format=%ct "$KERNEL_COMMIT")
kernel_cdate=$(LC_ALL=C TZ=UTC git -C "$work/src/kernel" log -1 --format=%cd --date=format-local:'%b %e %Y' "$KERNEL_COMMIT")
kernel_ctime=$(LC_ALL=C TZ=UTC git -C "$work/src/kernel" log -1 --format=%cd --date=format-local:'%H:%M:%S' "$KERNEL_COMMIT")
kernel_date=$(LC_ALL=C TZ=UTC git -C "$work/src/kernel" log -1 --format=%cd --date=format-local:'%a %b %e %H:%M:%S UTC %Y' "$KERNEL_COMMIT")
"$engine" run --rm -e HAKCHI_COMMIT="$HAKCHI_COMMIT" -e KERNEL_DATE="$kernel_date" -e KERNEL_EPOCH="$kernel_epoch" -e KERNEL_CDATE="$kernel_cdate" -e KERNEL_CTIME="$kernel_ctime" -e TZ=UTC \
  -v "$repo:/repo:ro,z" -v "$work:/work:z" "$IMAGE" /repo/scripts/kernel/build.sh --inside
