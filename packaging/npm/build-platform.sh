#!/usr/bin/env bash

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"

triple_to_pkg() {
  case "$1" in
    x86_64-unknown-linux-gnu)  echo "cruft-linux-x64 " ;;
    aarch64-unknown-linux-gnu) echo "cruft-linux-arm64 " ;;
    x86_64-apple-darwin)       echo "cruft-darwin-x64 " ;;
    aarch64-apple-darwin)      echo "cruft-darwin-arm64 " ;;
    x86_64-pc-windows-msvc)    echo "cruft-win32-x64 .exe" ;;
    *) echo "" ;;
  esac
}

arg="${1:?usage: build-platform.sh host | <rust-triple> <binary-path>}"
if [ "$arg" = "host" ]; then
  triple="$(rustc -vV | awk '/host:/{print $2}')"
  src="$REPO/target/release/cruft"
else
  triple="$arg"
  src="${2:?explicit triple requires a binary path}"
fi

mapping="$(triple_to_pkg "$triple")"
if [ -z "$mapping" ]; then
  echo "build-platform: unsupported triple '$triple'" >&2
  exit 2
fi
pkgdir="$(echo "$mapping" | cut -d' ' -f1)"
ext="$(echo "$mapping" | cut -d' ' -f2)"
dest="$HERE/platforms/$pkgdir/cruft$ext"
cpx_dest="$HERE/platforms/$pkgdir/cpx$ext"

[ -f "$src" ] || { echo "build-platform: binary not found at $src" >&2; exit 1; }
cp "$src" "$dest"
chmod +x "$dest"
rm -f "$cpx_dest"
echo "build-platform: $triple -> platforms/$pkgdir/cruft$ext ($(du -h "$dest" | cut -f1)); cpx exposed by package-manager command shim"
