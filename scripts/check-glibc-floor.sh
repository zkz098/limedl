#!/usr/bin/env bash
# Fail if a built ELF requires a GLIBC symbol version newer than a floor.
#
# The Linux desktop release is linked with
#   cargo zigbuild --target x86_64-unknown-linux-gnu.2.17
# which keeps the binary dynamically linked against the distro's
# fontconfig/X11/Wayland/GL but resolves the binary's *own* libc references
# against glibc 2.17 (Rust's minimum for the gnu target). A future dependency
# that references a newer versioned symbol would silently raise the floor
# again, so the release asserts the upper bound here.
#
# Usage:
#   check-glibc-floor.sh --binary <path/to/elf> [--max 2.17]
set -euo pipefail

BINARY=""
MAX="2.17"

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --binary) BINARY="$2"; shift 2 ;;
    --max)    MAX="$2";    shift 2 ;;
    -h|--help)
      sed -n '2,13p' "$0"
      exit 0 ;;
    *)
      echo "error: unknown argument '$1'" >&2
      exit 2 ;;
  esac
done

if [[ -z "$BINARY" || ! -f "$BINARY" ]]; then
  echo "error: --binary must point to an existing file (got '${BINARY}')" >&2
  exit 2
fi

if ! command -v objdump >/dev/null 2>&1; then
  echo "error: objdump (binutils) is required; install it or use a runner that has it" >&2
  exit 2
fi

FLOOR="GLIBC_${MAX}"
# `objdump -T` lists every dynamic symbol; keep only the GLIBC version tags and
# order them numerically (`sort -V` orders GLIBC_2.7 < GLIBC_2.17 < GLIBC_2.39).
REQUIRED="$(objdump -T "$BINARY" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n1 || true)"

if [[ -z "$REQUIRED" ]]; then
  echo "no GLIBC symbol versions found in $BINARY (static binary?): OK"
  exit 0
fi

NEWEST="$(printf '%s\n' "$FLOOR" "$REQUIRED" | sort -V | tail -n1)"
if [[ "$NEWEST" != "$FLOOR" ]]; then
  echo "error: $BINARY requires $REQUIRED, newer than the $FLOOR floor" >&2
  echo "       a dependency pulled in a newer libc symbol; find it with:" >&2
  echo "         objdump -T '$BINARY' | grep '$REQUIRED'" >&2
  exit 1
fi

echo "$BINARY requires at most $REQUIRED (floor $FLOOR): OK"
