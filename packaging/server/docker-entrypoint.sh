#!/bin/sh
# Container entrypoint for limedl-server.
#
# Mirrors the common NAS container convention: when started as root, fix the
# ownership of the mounted data/download directories to PUID:PGID and drop
# privileges with su-exec. When started as a non-root user, run as-is.
set -eu

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"
DATA_DIR="/var/lib/limedl"
DOWNLOAD_DIR="/downloads"

if [ "$(id -u)" = "0" ]; then
    mkdir -p "$DATA_DIR" "$DOWNLOAD_DIR"
    chown -R "$PUID:$PGID" "$DATA_DIR" "$DOWNLOAD_DIR" 2>/dev/null || true
    exec su-exec "$PUID:$PGID" limedl-server "$@"
fi

exec limedl-server "$@"