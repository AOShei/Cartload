#!/usr/bin/env bash
# Builds the console userland from source (docs/userland.md): the programs and libraries
# hakchi v1.0.4-126 puts in its boot ramdisk and package, with the toolchain they were built with.
#
#   scripts/userland/build.sh            build into target/userland/out
#   scripts/userland/build.sh --clean    remove target/userland first (the toolchain too)
#
# Needs podman or docker and network access for the sources. Everything runs in the container
# (Containerfile). The toolchain (Buildroot 2015.11.1, toolchain_defconfig) is built once into
# target/userland and reused.
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

  if [ ! -x "$br/output/host/usr/bin/${cross}gcc" ]; then
    echo "== toolchain (Buildroot 2015.11.1, GCC 4.9.3, glibc 2.22)"
    rm -rf "$br"
    tar -xjf "/work/dl/$BUILDROOT.tar.bz2" -C /work
    cp "$here/toolchain_defconfig" "$br/configs/cartload_toolchain_defconfig"
    make -s -C "$br" cartload_toolchain_defconfig >/dev/null
    make -C "$br" BR2_DL_DIR=/work/br-dl BR2_JLEVEL="$jobs" toolchain >/work/toolchain.log 2>&1 \
      || { tail -30 /work/toolchain.log >&2; exit 1; }
  fi
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

  echo "== done"
  ls -l "$out/bin" "$out/lib"
  exit 0
fi

# ---- on the host ----
repo=$(cd "$(dirname "$0")/../.." && pwd)
work=$repo/target/userland
[ "${1:-}" = "--clean" ] && rm -rf "$work"
engine=$(command -v podman || command -v docker || true)
[ -n "$engine" ] || { echo "needs podman or docker" >&2; exit 1; }
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
      ;;
    ''|'#'*) ;;
    *) echo "sources.txt: unknown kind $kind" >&2; exit 1 ;;
  esac
done < <(grep -vE '^\s*(#|$)' "$repo/scripts/userland/sources.txt")

echo "== build image"
"$engine" build -q -t "$IMAGE" -f "$repo/scripts/userland/Containerfile" "$repo/scripts/userland" >/dev/null
"$engine" run --rm -e TERM=dumb -e TZ=UTC -v "$repo:/repo:ro,z" -v "$work:/work:z" "$IMAGE" \
  /repo/scripts/userland/build.sh --inside
