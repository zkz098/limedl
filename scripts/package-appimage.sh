#!/usr/bin/env bash
# Package a limedl-native AppImage.
#
# Generates a standalone, relocatable Linux AppImage containing:
#   - AppRun entrypoint script
#   - limedl-native binary
#   - Desktop entry and icons
#
# Usage:
#   package-appimage.sh --version <x.y.z> --binary <path/to/limedl-native> \
#                       [--out-dir dist] [--arch <label>] [--icon <path/to/icon.png>]
set -euo pipefail

VERSION=""
BINARY=""
OUT_DIR="dist"
ARCH_LABEL=""
ICON=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)  VERSION="$2";    shift 2 ;;
    --binary)   BINARY="$2";     shift 2 ;;
    --out-dir)  OUT_DIR="$2";    shift 2 ;;
    --arch)     ARCH_LABEL="$2"; shift 2 ;;
    --icon)     ICON="$2";       shift 2 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0 ;;
    *)
      echo "error: unknown argument '$1'" >&2
      exit 2 ;;
  esac
done

if [ -z "$VERSION" ] || [ -z "$BINARY" ]; then
  echo "error: --version and --binary are required" >&2
  exit 2
fi
if [ ! -f "$BINARY" ]; then
  echo "error: built binary not found: $BINARY" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -z "$ICON" ]; then
  ICON="$REPO_ROOT/crates/limedl-native/ui/assets/icon.png"
fi

if [ -z "$ARCH_LABEL" ]; then
  case "$(uname -m)" in
    x86_64 | amd64) ARCH_LABEL="x86_64" ;;
    aarch64 | arm64) ARCH_LABEL="aarch64" ;;
    *) ARCH_LABEL="$(uname -m)" ;;
  esac
fi

ASSET="limedl-native-v${VERSION}-linux-${ARCH_LABEL}.AppImage"

APPDIR="$(mktemp -d)"
trap 'rm -rf "$APPDIR"' EXIT

# ── 1. Executables ───────────────────────────────────────────────────────────
mkdir -p "$APPDIR/usr/bin"
install -m 0755 "$BINARY" "$APPDIR/usr/bin/limedl-native"
ln -sf "limedl-native" "$APPDIR/usr/bin/limedl"

# ── 2. AppRun ────────────────────────────────────────────────────────────────
APPRUN_SRC="$REPO_ROOT/packaging/linux/AppRun"
if [ ! -f "$APPRUN_SRC" ]; then
  echo "error: AppRun template not found: $APPRUN_SRC" >&2
  exit 1
fi
install -m 0755 "$APPRUN_SRC" "$APPDIR/AppRun"

# ── 3. Desktop Entry ─────────────────────────────────────────────────────────
DESKTOP_SRC="$REPO_ROOT/packaging/linux/limedl-native.desktop"
if [ ! -f "$DESKTOP_SRC" ]; then
  echo "error: desktop file template not found: $DESKTOP_SRC" >&2
  exit 1
fi
install -m 0644 "$DESKTOP_SRC" "$APPDIR/limedl-native.desktop"
mkdir -p "$APPDIR/usr/share/applications"
install -m 0644 "$DESKTOP_SRC" "$APPDIR/usr/share/applications/limedl-native.desktop"

# ── 4. Icons ─────────────────────────────────────────────────────────────────
install -m 0644 "$ICON" "$APPDIR/limedl-native.png"
ln -sf "limedl-native.png" "$APPDIR/.DirIcon"

mkdir -p "$APPDIR/usr/share/pixmaps"
install -m 0644 "$ICON" "$APPDIR/usr/share/pixmaps/limedl-native.png"

mkdir -p "$APPDIR/usr/share/icons/hicolor/512x512/apps"
install -m 0644 "$ICON" "$APPDIR/usr/share/icons/hicolor/512x512/apps/limedl-native.png"

ICON_32="$REPO_ROOT/crates/limedl-native/ui/assets/32x32.png"
if [ -f "$ICON_32" ]; then
  mkdir -p "$APPDIR/usr/share/icons/hicolor/32x32/apps"
  install -m 0644 "$ICON_32" "$APPDIR/usr/share/icons/hicolor/32x32/apps/limedl-native.png"
fi

if command -v convert >/dev/null 2>&1; then
  for s in 16 48 64 128 256; do
    mkdir -p "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps"
    convert "$ICON" -resize "${s}x${s}" "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps/limedl-native.png" 2>/dev/null || true
  done
elif command -v python3 >/dev/null 2>&1 && python3 -c "from PIL import Image" >/dev/null 2>&1; then
  python3 -c "
from PIL import Image
import os, sys
src = sys.argv[1]
dest = sys.argv[2]
try:
    im = Image.open(src)
    for s in [16, 48, 64, 128, 256]:
        d = os.path.join(dest, f'usr/share/icons/hicolor/{s}x{s}/apps')
        os.makedirs(d, exist_ok=True)
        im.resize((s, s), Image.Resampling.LANCZOS).save(os.path.join(d, 'limedl-native.png'))
except Exception as e:
    pass
" "$ICON" "$APPDIR"
fi

# ── 5. AppImageTool Execution ────────────────────────────────────────────────
APPIMAGETOOL=""
if command -v appimagetool >/dev/null 2>&1; then
  APPIMAGETOOL="appimagetool"
else
  TOOL_CACHE="${RUNNER_TEMP:-/tmp}/appimagetool-cache"
  TOOL_PATH="$TOOL_CACHE/appimagetool-${ARCH_LABEL}"
  if [ ! -f "$TOOL_PATH" ]; then
    mkdir -p "$TOOL_CACHE"
    URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH_LABEL}.AppImage"
    echo "downloading appimagetool from $URL..."
    curl -fsSL --retry 3 -o "$TOOL_PATH" "$URL"
    chmod +x "$TOOL_PATH"
  fi
  APPIMAGETOOL="$TOOL_PATH"
fi

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR/$ASSET"

# --appimage-extract-and-run allows running inside containers / CI without FUSE.
ARCH="$ARCH_LABEL" "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$OUT_DIR/$ASSET"
chmod +x "$OUT_DIR/$ASSET"

echo "wrote $OUT_DIR/$ASSET"
ls -lh "$OUT_DIR/$ASSET"
