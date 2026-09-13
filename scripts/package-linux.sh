#!/usr/bin/env bash
# Package a limedl-native Linux release: the binary plus a short README, in a
# single tar.gz that also unpacks tidily from a file manager.
#
# The archive deliberately has one top-level directory (`limedl-native/`). A
# tarball whose members sit at the root scatters a binary and a README into the
# user's current directory when they extract it — and on Linux the release asset
# is usually extracted by hand, unlike the macOS `.app` half.
#
# The self-updater matches the binary by file name anywhere in the archive
# (update.rs, `extract_executable`), so the nested path is fine.
#
# Usage:
#   package-linux.sh --version <x.y.z> --binary <path/to/limedl-native> \
#                    [--out-dir dist] [--arch <label>]
set -euo pipefail

VERSION=""
BINARY=""
OUT_DIR="dist"
ARCH_LABEL=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)  VERSION="$2";    shift 2 ;;
    --binary)   BINARY="$2";     shift 2 ;;
    --out-dir)  OUT_DIR="$2";    shift 2 ;;
    --arch)     ARCH_LABEL="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,18p' "$0"
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

if [ -z "$ARCH_LABEL" ]; then
  case "$(uname -m)" in
    x86_64 | amd64) ARCH_LABEL="x86_64" ;;
    aarch64 | arm64) ARCH_LABEL="aarch64" ;;
    *) ARCH_LABEL="$(uname -m)" ;;
  esac
fi

PACKAGE_NAME="limedl-native"
ASSET="limedl-native-v${VERSION}-linux-${ARCH_LABEL}-portable.tar.gz"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/$PACKAGE_NAME"
install -m 0755 "$BINARY" "$STAGE/$PACKAGE_NAME/limedl-native"

cat > "$STAGE/$PACKAGE_NAME/README.txt" <<EOF
limedl ${VERSION} — Linux desktop build (${ARCH_LABEL})

Install
  mkdir -p ~/.local/bin
  cp limedl-native ~/.local/bin/
  # add ~/.local/bin to PATH if it is not already there

Run
  limedl-native            # opens the main window
  limedl-native --hidden   # start in the tray only

Requirements
  - A system tray (StatusNotifier / appindicator host). On GNOME install the
    AppIndicator shell extension; Debian/Ubuntu also need
    libayatana-appindicator3-1.
  - xdg-desktop-portal (usually present) for the file and folder pickers.

Autostart is enabled from Settings and writes
~/.config/autostart/limedl-native.desktop
EOF

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR/$ASSET"
# --owner/--group reset the build user; GNU tar is what the Linux release job runs.
tar --owner=0 --group=0 -czf "$OUT_DIR/$ASSET" -C "$STAGE" "$PACKAGE_NAME"

echo "wrote $OUT_DIR/$ASSET"
tar -tzf "$OUT_DIR/$ASSET"
