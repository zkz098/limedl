#!/usr/bin/env bash
# Package a limedl-native Debian (.deb) package.
#
# Generates a standard Debian binary package containing:
#   - /usr/bin/limedl-native (and /usr/bin/limedl symlink)
#   - /usr/share/applications/limedl-native.desktop
#   - /usr/share/icons/hicolor/*/apps/limedl-native.png
#   - /usr/share/pixmaps/limedl-native.png
#
# Usage:
#   package-deb.sh --version <x.y.z> --binary <path/to/limedl-native> \
#                  [--out-dir dist] [--arch <label>] [--icon <path/to/icon.png>]
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
      sed -n '2,13p' "$0"
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

DEB_ARCH="amd64"
case "$ARCH_LABEL" in
  x86_64 | amd64) DEB_ARCH="amd64" ;;
  aarch64 | arm64) DEB_ARCH="arm64" ;;
  *) DEB_ARCH="$ARCH_LABEL" ;;
esac

ASSET="limedl-native-v${VERSION}-linux-${ARCH_LABEL}.deb"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# ── 1. Executables ───────────────────────────────────────────────────────────
mkdir -p "$STAGE/usr/bin"
install -m 0755 "$BINARY" "$STAGE/usr/bin/limedl-native"
ln -sf "limedl-native" "$STAGE/usr/bin/limedl"

# ── 2. Desktop Entry ─────────────────────────────────────────────────────────
DESKTOP_SRC="$REPO_ROOT/packaging/linux/limedl-native.desktop"
if [ ! -f "$DESKTOP_SRC" ]; then
  echo "error: desktop file template not found: $DESKTOP_SRC" >&2
  exit 1
fi
mkdir -p "$STAGE/usr/share/applications"
install -m 0644 "$DESKTOP_SRC" "$STAGE/usr/share/applications/limedl-native.desktop"

# ── 3. Icons ─────────────────────────────────────────────────────────────────
mkdir -p "$STAGE/usr/share/pixmaps"
install -m 0644 "$ICON" "$STAGE/usr/share/pixmaps/limedl-native.png"

mkdir -p "$STAGE/usr/share/icons/hicolor/512x512/apps"
install -m 0644 "$ICON" "$STAGE/usr/share/icons/hicolor/512x512/apps/limedl-native.png"

ICON_32="$REPO_ROOT/crates/limedl-native/ui/assets/32x32.png"
if [ -f "$ICON_32" ]; then
  mkdir -p "$STAGE/usr/share/icons/hicolor/32x32/apps"
  install -m 0644 "$ICON_32" "$STAGE/usr/share/icons/hicolor/32x32/apps/limedl-native.png"
fi

if command -v convert >/dev/null 2>&1; then
  for s in 16 48 64 128 256; do
    mkdir -p "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps"
    convert "$ICON" -resize "${s}x${s}" "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps/limedl-native.png" 2>/dev/null || true
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
" "$ICON" "$STAGE"
fi

# ── 4. Control File ──────────────────────────────────────────────────────────
mkdir -p "$STAGE/DEBIAN"
INSTALLED_SIZE=$(du -ks "$STAGE/usr" 2>/dev/null | cut -f1 || echo 30000)

cat > "$STAGE/DEBIAN/control" <<EOF
Package: limedl-native
Version: ${VERSION}
Section: net
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: zkz098 <https://github.com/zkz098/limedl>
Installed-Size: ${INSTALLED_SIZE}
Depends: libc6, libgtk-3-0, libayatana-appindicator3-1 | libappindicator3-1, xdg-desktop-portal
Recommends: gnome-shell-extension-appindicator
Homepage: https://github.com/zkz098/limedl
Description: Lightweight native desktop download manager based on Slint
 limedl is a high-performance modern download manager featuring multi-threaded
 chunked HTTP/HTTPS downloads, BitTorrent support, aria2 RPC compatibility,
 and a fast Slint-based native desktop interface.
EOF

# ── 5. Build .deb ────────────────────────────────────────────────────────────
if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "error: dpkg-deb is required to package deb files" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR/$ASSET"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT_DIR/$ASSET"

echo "wrote $OUT_DIR/$ASSET"
dpkg-deb -I "$OUT_DIR/$ASSET"
