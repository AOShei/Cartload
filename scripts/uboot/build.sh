#!/usr/bin/env bash
# Builds the console's u-boot (boot/uboot.bin) from source (docs/uboot.md): Nintendo's r16-uboot
# from the NES Classic's open-source archive, hakchi's patch (the hakchi fork's r16-uboot.diff),
# Cartload's patches (patches/), with the toolchain of scripts/userland/build.sh, and the board
# configuration compiled from sys_config.fex; joined as on a console: u-boot, padded to 0x80000,
# the configuration, padded to 0xba000 (the length the copies in the NAND boot area are spaced
# by), header lengths and checksum.
#
#   scripts/uboot/build.sh            into target/uboot/out: uboot.bin, u-boot.map, report.txt
#
# Needs scripts/userland/build.sh's toolchain and its hakchi checkout (target/userland).
set -euo pipefail

ZIP=NintendoEntertainmentSystemNESClassicEdition_OSS.zip
ZIP_URL=http://data.nintendo.co.jp/oss/$ZIP
ZIP_SHA256=d6cfa41888185889c2b580d91ae4a8b07f7e1b737334110ee50173f20386187b
UBOOT=r16-uboot-fc3061df4dbd4153819b2d2f141d82b88fea51cf
UBOOT_SHA256=3afd5abcebc7e18b4de2923c51f48eea2f4cca5d7bcba589f44dac51c7ae283f
IMAGE_LEN=$((0xba000)) # u-boot and its configuration: where the boot-info starts in each copy
SCRIPT_AT=$((0x80000)) # where Nintendo's u-boot has its configuration

if [ "${1:-}" = "--inside" ]; then
  # ---- inside the container: /repo the checkout, /work target/userland, /out target/uboot ----
  export PATH=/work/buildroot-2015.11.1/output/host/usr/bin:$PATH
  cross=arm-buildroot-linux-gnueabihf-
  here=/repo/scripts/uboot
  hakchi=/work/src/hakchi
  out=/out/out
  rm -rf /out/build "$out" /out/host
  mkdir -p /out/build "$out" /out/host

  echo "== source: $UBOOT, hakchi's patch, Cartload's patches"
  tar -xzf "/out/dl/$UBOOT.tar.gz" -C /out/build
  src=/out/build/$UBOOT
  patch -d "$src" -p1 -s <"$hakchi/r16-uboot/r16-uboot.diff"
  chmod +x "$src/fake_git"
  for p in "$here"/patches/*.patch; do patch -d "$src" -p1 -s <"$p"; done

  # The version banner records the build date: the hakchi checkout's commit time instead, so a
  # build of the same sources gives the same bytes.
  epoch=$(git -C "$hakchi" log -1 --format=%ct)
  mkdir -p /out/host/shim
  printf '#!/bin/sh\nexec /bin/date -d @%s "$@"\n' "$epoch" >/out/host/shim/date
  chmod +x /out/host/shim/date

  echo "== u-boot ($(${cross}gcc --version | head -1))"
  # u-boot only: boot0 and fes1 (the SPLs) link Allwinner's DRAM setup, a binary.
  (cd "$src" && export PATH="/out/host/shim:$PATH" \
    && make ARCH=arm CROSS_COMPILE="$cross" distclean >/dev/null \
    && make ARCH=arm CROSS_COMPILE="$cross" clover_config >/dev/null \
    && make ARCH=arm CROSS_COMPILE="$cross" -B -j"$(nproc)" >/out/build.log 2>&1) \
    || { tail -30 /out/build.log >&2; exit 1; }
  if grep -qiE 'openssl|libcrypto|libssl|libdram' "$src/u-boot.map"; then
    echo "u-boot.map: a prebuilt library was linked" >&2
    exit 1
  fi

  echo "== host tools: sunxi-fexc, sntool"
  cp -a /work/src/sunxi-tools-1e219c0 /out/host/sunxi-tools
  make -s -C /out/host/sunxi-tools sunxi-fexc >/dev/null
  gcc -std=gnu99 -O2 -I"$hakchi/sntool" -I"$hakchi/3rdparty/sunxi-tools" -I"$hakchi/3rdparty/mkbootimg" \
    -o /out/host/sntool "$hakchi"/sntool/*.c -lm

  echo "== uboot.bin"
  /out/host/sunxi-tools/sunxi-fexc -q -I fex -O bin "$here/sys_config.fex" /out/build/script.bin
  ulen=$(stat -c %s "$src/u-boot.bin")
  slen=$(stat -c %s /out/build/script.bin)
  [ "$ulen" -le "$SCRIPT_AT" ] || { echo "u-boot is $ulen bytes, more than $SCRIPT_AT" >&2; exit 1; }
  [ $((SCRIPT_AT + slen)) -le "$IMAGE_LEN" ] || { echo "the configuration does not fit" >&2; exit 1; }
  img=$out/uboot.bin
  # 0xff (erased NAND) around both, as in Nintendo's image.
  head -c "$IMAGE_LEN" /dev/zero | tr '\0' '\377' >"$img"
  dd if="$src/u-boot.bin" of="$img" conv=notrunc status=none
  dd if=/out/build/script.bin of="$img" bs=1 seek="$SCRIPT_AT" conv=notrunc status=none
  le32() { printf "\\x$(printf %02x $(($1 & 255)))\\x$(printf %02x $((($1 >> 8) & 255)))\\x$(printf %02x $((($1 >> 16) & 255)))\\x$(printf %02x $((($1 >> 24) & 255)))"; }
  le32 "$IMAGE_LEN" | dd of="$img" bs=1 seek=$((0x14)) conv=notrunc status=none
  le32 "$SCRIPT_AT" | dd of="$img" bs=1 seek=$((0x18)) conv=notrunc status=none
  /out/host/sntool check -f "$img" >/dev/null
  /out/host/sntool check "$img" >/dev/null || { echo "uboot.bin: checksum" >&2; exit 1; }
  cp "$src/u-boot.map" "$out/"
  {
    echo "# u-boot: $UBOOT + hakchi $(git -C "$hakchi" rev-parse --short HEAD) r16-uboot.diff + Cartload's patches"
    echo "# $(${cross}gcc --version | head -1)"
    strings -n 8 "$img" | grep -o -m1 'U-Boot 20.*' || true
    strings -n 8 "$img" | grep -m1 '^bootcmd=' || true
    echo "u-boot $ulen bytes, configuration $slen bytes at $(printf 0x%x $SCRIPT_AT), image $(printf 0x%x $IMAGE_LEN)"
    sha256sum "$img" | sed "s|$out/||"
  } >"$out/report.txt"
  cat "$out/report.txt"
  exit 0
fi

# ---- on the host ----
repo=$(cd "$(dirname "$0")/../.." && pwd)
work=$repo/target/uboot
engine=$(command -v podman || command -v docker || true)
[ -n "$engine" ] || { echo "needs podman or docker" >&2; exit 1; }
[ -x "$repo/target/userland/buildroot-2015.11.1/output/host/usr/bin/arm-buildroot-linux-gnueabihf-gcc" ] \
  || { echo "no toolchain: run scripts/userland/build.sh first" >&2; exit 1; }
mkdir -p "$work/dl"

echo "== sources"
if [ ! -f "$work/dl/$UBOOT.tar.gz" ]; then
  if [ ! -f "$work/dl/$ZIP" ]; then
    curl -sSLf --max-time 1800 -o "$work/dl/$ZIP.partial" "$ZIP_URL"
    mv -f "$work/dl/$ZIP.partial" "$work/dl/$ZIP"
  fi
  echo "$ZIP_SHA256  $work/dl/$ZIP" | sha256sum -c --quiet - || { echo "$ZIP: checksum mismatch" >&2; exit 1; }
  unzip -o -q -j "$work/dl/$ZIP" "NintendoEntertainmentSystemNESClassicEdition_OSS/$UBOOT.tar.gz" -d "$work/dl"
fi
echo "$UBOOT_SHA256  $work/dl/$UBOOT.tar.gz" | sha256sum -c --quiet - || { echo "$UBOOT: checksum mismatch" >&2; exit 1; }

"$engine" run --rm -e TZ=UTC -v "$repo:/repo:ro,z" -v "$repo/target/userland:/work:ro,z" -v "$work:/out:z" \
  cartload-userland-build /repo/scripts/uboot/build.sh --inside
