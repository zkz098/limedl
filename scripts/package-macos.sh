#!/usr/bin/env bash
# Assemble, ad-hoc sign and archive the limedl-native macOS app bundle.
#
# Run on macOS (codesign/cputype/tar are OS tooling). CI calls this from the
# `build-native-macos` job in .github/workflows/release.yml; it is equally usable
# locally to produce a runnable bundle from a plain `cargo build`.
#
# Why a bundle instead of the raw binary: a bare Mach-O started from the terminal
# is not a proper app. It gets no Dock tile, `NSApplication` never becomes
# active, notifications fall back to an unattributed agent, and `tray-icon`
# cannot install a status item that survives the window losing focus. The
# .app/Contents/MacOS layout is also what the self-updater expects — it re-signs
# the bundle it finds around the running executable (update.rs).
#
# Signing is ad-hoc (`--sign -`): no Apple Developer account is needed, and the
# app runs on the machine that built/downloaded it. It is NOT notarized, so a
# copy downloaded through a browser carries `com.apple.quarantine` and the first
# launch needs right-click → Open. Swapping in a Developer ID identity later only
# changes SIGN_IDENTITY below (plus a notarytool step).
#
# Usage:
#   package-macos.sh --version <x.y.z> --binary <path/to/limedl-native> \
#                    [--out-dir dist] [--icon <path/to/icon.png>] [--arch <label>]
set -euo pipefail

SIGN_IDENTITY="${SIGN_IDENTITY:--}"

VERSION=""
BINARY=""
OUT_DIR="dist"
ICON=""
ARCH_LABEL=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)  VERSION="$2";    shift 2 ;;
    --binary)   BINARY="$2";     shift 2 ;;
    --out-dir)  OUT_DIR="$2";    shift 2 ;;
    --icon)     ICON="$2";       shift 2 ;;
    --arch)     ARCH_LABEL="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,25p' "$0"
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
TEMPLATE="$REPO_ROOT/packaging/macos/Info.plist.in"
if [ ! -f "$TEMPLATE" ]; then
  echo "error: Info.plist template not found: $TEMPLATE" >&2
  exit 1
fi

if [ -z "$ARCH_LABEL" ]; then
  # `cputype` is the macOS equivalent of `file -b` and reports arm64/x86_64 for
  # a Mach-O; `uname -m` does the same for native builds.
  ARCH_LABEL="$(uname -m)"
fi

APP_NAME="limedl.app"
BUNDLE="$OUT_DIR/$APP_NAME"
ASSET="limedl-native-v${VERSION}-darwin-${ARCH_LABEL}-portable.tar.gz"

rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources" "$OUT_DIR"

# ── Binary ──────────────────────────────────────────────────────────────────
# CFBundleExecutable in Info.plist must match this file name.
install -m 0755 "$BINARY" "$BUNDLE/Contents/MacOS/limedl-native"

# ── Info.plist (version substituted) ────────────────────────────────────────
# `plutil -lint` catches a malformed template here instead of at first launch,
# where an unreadable Info.plist makes the bundle silently non-launchable.
sed "s/__VERSION__/${VERSION}/g" "$TEMPLATE" > "$BUNDLE/Contents/Info.plist"
plutil -lint "$BUNDLE/Contents/Info.plist"

# ── Icon ────────────────────────────────────────────────────────────────────
# Optional: without --icon the bundle keeps the generic app icon.
if [ -n "$ICON" ]; then
  bash "$SCRIPT_DIR/make-macos-icon.sh" "$ICON" "$BUNDLE/Contents/Resources/AppIcon.icns"
fi

# ── Ad-hoc signature ────────────────────────────────────────────────────────
# Sign the bundle (not just the binary): macOS validates the nested code of the
# whole bundle, and a binary-only signature leaves the bundle unlaunchable when
# a resource changes. `--force` makes re-runs idempotent, `--sign -` selects the
# ad-hoc identity (empty certificate chain, no keychain entry).
codesign --force --sign "$SIGN_IDENTITY" --timestamp=none "$BUNDLE"
codesign --verify --strict "$BUNDLE"
echo "signed $BUNDLE (identity: $SIGN_IDENTITY)"

# ── Archive ─────────────────────────────────────────────────────────────────
# Plain `tar -czf`: bsdtar is what macOS ships and it round-trips the bundle
# layout. Keep the binary as a top-level tar member name so the updater's
# member match (update.rs, extract_executable) finds it.
rm -f "$OUT_DIR/$ASSET"
tar -czf "$OUT_DIR/$ASSET" -C "$OUT_DIR" "$APP_NAME"

echo "wrote $OUT_DIR/$ASSET"
ls -la "$BUNDLE/Contents" "$BUNDLE/Contents/MacOS"
