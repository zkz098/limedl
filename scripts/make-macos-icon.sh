#!/usr/bin/env bash
# Convert a PNG into the AppIcon.icns that scripts/package-macos.sh puts in the
# bundle's Contents/Resources.
#
# macOS-only by design: `sips` and `iconutil` ship with the OS, so the release
# job needs no extra tooling and no icon file has to be committed. Generates the
# complete iconset matrix (the same set Xcode emits) rather than the single
# 512x512 an `sips -s format icns` shortcut would produce — Finder, the Dock and
# the About window pick different members and fall back to a blurry upscale when
# a size is missing.
#
# Usage: make-macos-icon.sh <source.png> <out.icns>
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <source.png> <out.icns>" >&2
  exit 2
fi

SRC="$1"
OUT="$2"

if [ ! -f "$SRC" ]; then
  echo "error: source image not found: $SRC" >&2
  exit 1
fi
for tool in sips iconutil; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: '$tool' not found — this script must run on macOS" >&2
    exit 1
  fi
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"

# `sips -z <height> <width>`; every iconset member is square and 1x/2x pairs are
# the same pixel data under a different name.
for base in 16 32 128 256 512; do
  sips -z "$base" "$base" "$SRC" --out "$ICONSET/icon_${base}x${base}.png" >/dev/null
  double=$((base * 2))
  sips -z "$double" "$double" "$SRC" --out "$ICONSET/icon_${base}x${base}@2x.png" >/dev/null
done

mkdir -p "$(dirname "$OUT")"
iconutil -c icns "$ICONSET" -o "$OUT"
echo "wrote $OUT"
