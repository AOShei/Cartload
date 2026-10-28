#!/usr/bin/env bash
# Builds the console userland from source (docs/userland.md): the programs and libraries
# hakchi v1.0.4-126 puts in its boot ramdisk and package, with the toolchain they were built with.
#
#   scripts/userland/build.sh            build into target/userland/out
#   scripts/userland/build.sh --clean    remove target/userland first (the toolchain too)
#
# Needs podman or docker and network access for the sources. Everything runs in the container
# (Containerfile). Buildroot 2015.11.1 (buildroot_defconfig) builds the toolchain and the packages
# whose shipped version it carries, once, into target/userland; later runs reuse them.
#
# Sources are pinned: tarballs by SHA-256, git repositories by commit (sources.txt).
set -euo pipefail

IMAGE=cartload-userland-build
BUILDROOT=buildroot-2015.11.1

if [ "${1:-}" = "--inside" ]; then
  # ---- inside the container: /repo is the Cartload checkout, /work is target/userland ----
  jobs=$(nproc)
  here=/repo/scripts/userland
  br=/work/$BUILDROOT
  out=/work/out
  export PATH="$br/output/host/usr/bin:$PATH"
  cross=arm-buildroot-linux-gnueabihf-

  # Sources Buildroot builds with its own recipes (local.mk): unpacked fresh each run.
  for t in e2fsprogs-1.44.1.tar.gz dropbear-2018.76.tar.bz2 kexec-tools-2.0.15.tar.xz rsync-3.1.3.tar.gz; do
    rm -rf "/work/src/${t%.tar.*}"
    tar -xf "/work/dl/$t" -C /work/src
  done

  echo "== Buildroot 2015.11.1: toolchain (GCC 4.9.3, glibc 2.22) and its packages"
  if [ ! -d "$br" ]; then
    tar -xjf "/work/dl/$BUILDROOT.tar.bz2" -C /work
  fi
  # Recipe changes (buildroot-patches), each applied once.
  for p in "$here"/buildroot-patches/*.patch; do
    patch -d "$br" -p1 -R --dry-run -s -f <"$p" >/dev/null 2>&1 || patch -d "$br" -p1 -s <"$p"
  done
  cp "$here/buildroot_defconfig" "$br/configs/cartload_defconfig"
  make -s -C "$br" cartload_defconfig >/dev/null
  # Buildroot does not notice changed options of packages built from outside sources: those are
  # rebuilt from clean each run (they are small).
  for p in e2fsprogs dropbear kexec rsync; do
    make -s -C "$br" "$p-dirclean" >/dev/null
  done
  # Already built packages are skipped; the first run takes a few minutes.
  make -C "$br" BR2_DL_DIR=/work/br-dl BR2_JLEVEL="$jobs" >/work/buildroot.log 2>&1 \
    || { tail -30 /work/buildroot.log >&2; exit 1; }
  "${cross}gcc" --version | head -1

  rm -rf "$out"
  mkdir -p "$out/bin" "$out/lib"

  echo "== busybox"
  cd /work/src/busybox
  git clean -qfdx
  cp "$here/busybox.config" .config
  KCONFIG_NOTIMESTAMP=1 make -s oldconfig </dev/null >/dev/null
  make -s silentoldconfig >/dev/null
  # The banner reads "BusyBox v1.29.0.git (<build time>)": the commit's time, so a rebuild
  # gives the same binary.
  sed -i "s|^#define AUTOCONF_TIMESTAMP .*|#define AUTOCONF_TIMESTAMP \"$(git log -1 --format=%cd \
    --date=format-local:'%Y-%m-%d %H:%M:%S UTC')\"|" include/autoconf.h
  make -j"$jobs" CROSS_COMPILE="$cross" busybox >/work/busybox.log 2>&1 \
    || { tail -30 /work/busybox.log >&2; exit 1; }
  cp busybox "$out/bin/busybox"

  # Pinned build times for the rest: a date that reports the commit time (for build stamps
  # made with `date`) and a header pinning __DATE__ and __TIME__.
  stamp() { # stamp <git dir>: sets epoch, the PATH shim and the header for that commit
    epoch=$(git -C "$1" log -1 --format=%ct)
    mkdir -p /work/shim
    printf '#!/bin/sh\nexec /bin/date -d @%s "$@"\n' "$epoch" >/work/shim/date
    chmod +x /work/shim/date
    printf '#define __DATE__ "%s"\n#define __TIME__ "%s"\n' \
      "$(LC_ALL=C TZ=UTC /bin/date -d "@$epoch" '+%b %e %Y')" \
      "$(TZ=UTC /bin/date -d "@$epoch" +%H:%M:%S)" >/work/shim/reproducible.h
  }
  pinned="-Wno-builtin-macro-redefined -include /work/shim/reproducible.h"
  cc="${cross}gcc"

  echo "== llmnrd ($(git -C /work/src/llmnrd describe --tags))"
  cd /work/src/llmnrd && git clean -qfdx
  make -s CC="$cc" >/work/llmnrd.log 2>&1 || { tail -20 /work/llmnrd.log >&2; exit 1; }
  cp llmnrd "$out/bin/"

  echo "== tinysvcmdns (geekman's library, Cartload's main)"
  cd /work/src/tinysvcmdns && git clean -qfdx
  "$cc" -std=gnu99 -O2 -DNDEBUG -Wall -o "$out/bin/tinysvcmdns" mdns.c mdnsd.c "$here/main-tinysvcmdns.c" -I. -lpthread

  echo "== clovershell"
  cd /work/src/clovershell-daemon && git clean -qfdx && stamp .
  "$cc" -O2 -Wall $pinned -o "$out/bin/clovershell" clovershell.c

  echo "== sunxi-tools"
  for t in 1e219c0:sunxi-fexc 1e219c0:sunxi-pio 5c19710:sunxi-nand-part; do
    cd "/work/src/sunxi-tools-${t%%:*}" && git clean -qfdx
    make -s CC="$cc" "${t#*:}" >/work/sunxi-tools.log 2>&1 || { tail -20 /work/sunxi-tools.log >&2; exit 1; }
    cp "${t#*:}" "$out/bin/"
  done

  echo "== sntool (public hakchi)"
  cd /work/src/hakchi/sntool && git -C /work/src/hakchi clean -qfdx -- sntool && stamp /work/src/hakchi
  PATH="/work/shim:$PATH" make -s GCC="$cc" >/work/sntool.log 2>&1 || { tail -20 /work/sntool.log >&2; exit 1; }
  cp sntool "$out/bin/"

  echo "== xdelta3 3.1.1"
  cd /work/src/xdelta/xdelta3 && git -C /work/src/xdelta clean -qfdx
  autoreconf -fi >/work/xdelta.log 2>&1
  # Its Makefile compiles as C99, where glibc 2.22 does not define static_assert; GCC 4.9
  # has _Static_assert there too.
  CPPFLAGS="-Dstatic_assert=_Static_assert" ./configure --host=arm-buildroot-linux-gnueabihf --without-liblzma \
    >>/work/xdelta.log 2>&1 \
    && make -s xdelta3 >>/work/xdelta.log 2>&1 || { tail -20 /work/xdelta.log >&2; exit 1; }
  cp xdelta3 "$out/bin/"

  echo "== done"
  ls -l "$out/bin" "$out/lib"
  exit 0
fi

# ---- on the host ----
repo=$(cd "$(dirname "$0")/../.." && pwd)
work=$repo/target/userland
engine=$(command -v podman || command -v docker || true)
[ -n "$engine" ] || { echo "needs podman or docker" >&2; exit 1; }
if [ "${1:-}" = "--clean" ]; then
  # Buildroot leaves read-only directories; with rootless podman the files belong to the
  # user namespace.
  if [ "$(basename "$engine")" = podman ]; then podman unshare rm -rf "$work"; else rm -rf "$work"; fi
fi
mkdir -p "$work/dl" "$work/br-dl" "$work/src"

echo "== sources"
# sources.txt: "file <name> <url> <sha256>" or "git <dir> <url> <commit>"
while read -r kind name url pin; do
  case "$kind" in
    file)
      f="$work/dl/$name"
      if [ ! -f "$f" ]; then
        curl -sSLf --max-time 900 -o "$f.partial" "$url"
        mv -f "$f.partial" "$f"
      fi
      echo "$pin  $f" | sha256sum -c --quiet - || { echo "$name: checksum mismatch" >&2; exit 1; }
      ;;
    git)
      d="$work/src/$name"
      [ -d "$d/.git" ] || git clone -q "$url" "$d"
      git -C "$d" cat-file -e "$pin^{commit}" 2>/dev/null || git -C "$d" fetch -q origin
      git -C "$d" checkout -q --force "$pin"
      # sntool needs headers from hakchi's mkbootimg submodule.
      if [ "$name" = hakchi ]; then
        git -C "$d" submodule update -q --init 3rdparty/mkbootimg 3rdparty/sunxi-tools
      fi
      ;;
    ''|'#'*) ;;
    *) echo "sources.txt: unknown kind $kind" >&2; exit 1 ;;
  esac
done < <(grep -vE '^\s*(#|$)' "$repo/scripts/userland/sources.txt")

echo "== build image"
"$engine" build -q -t "$IMAGE" -f "$repo/scripts/userland/Containerfile" "$repo/scripts/userland" >/dev/null
"$engine" run --rm -e TERM=dumb -e TZ=UTC -v "$repo:/repo:ro,z" -v "$work:/work:z" "$IMAGE" \
  /repo/scripts/userland/build.sh --inside
