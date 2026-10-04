---
type: operations
title: Server Deployment and Packaging
description: The committed deployment assets for the headless daemon — the hardened systemd unit and env file, the container entrypoint with its PUID/PGID convention, the from-source and prebuilt-binary Dockerfiles, the compose example and the multi-arch GHCR image.
tags: [operations, server, systemd, docker, packaging, nas]
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T12:58:25.182Z
sources:
  - id: openwiki-source-715dace563ef484b6e8bd1e2
    resource: repo://.dockerignore
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-7ae9f72a70d7b68c2986edb0
    resource: repo://packaging/server/docker-compose.yml
  - id: openwiki-source-d968ee8eed27c8e3566040b5
    resource: repo://packaging/server/docker-entrypoint.sh
  - id: openwiki-source-7003b6883ff44b1304d949e8
    resource: repo://packaging/server/Dockerfile
  - id: openwiki-source-e3a033f825963df0acd533cc
    resource: repo://packaging/server/Dockerfile.release
  - id: openwiki-source-1884e5af4fddc20aeb601731
    resource: repo://packaging/server/systemd/limedl-server.env.example
  - id: openwiki-source-816a10881eb55b69eaf93236
    resource: repo://packaging/server/systemd/limedl-server.service
generated: { by: "pi", at: "2026-10-04T12:58:25.182Z" }
---

# Server Deployment and Packaging

The headless `limedl-server` daemon is deployed in one of two ways: as a native
service managed by systemd, or as a container. Both are driven by files committed
under `packaging/server/`, and the same assets ship in the release tarball so a
NAS install is one download. Engine/CLI behaviour is on
[Headless Server Daemon](../integrations/headless-server-daemon.md); the
step-by-step install narrative is `docs/server-daemon.md`.

## systemd

`packaging/server/systemd/limedl-server.service` is a `Type=simple` unit that:

- runs as a dedicated `limedl` user/group (the account is created by the
  operator, not the unit);
- reads secrets from `/etc/limedl-server.env` through `EnvironmentFile`, so the
  token is not visible in the unit body or the process list;
- starts `limedl-server --rpc-listen 0.0.0.0 --data-dir /var/lib/limedl
  --download-dir /srv/downloads` and restarts on failure.

It applies `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome=true`,
`PrivateTmp=true`, `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX` and a
`ReadWritePaths` allow-list of `/var/lib/limedl` and `/srv/downloads`. Those two
paths are the whole writable surface the daemon needs: the SQLite database,
torrent state, logs and buffered files all live under the data directory, and the
BitTorrent output directory is `<data-dir>/downloads/bt_files`. A custom
`--download-dir` must be added to `ReadWritePaths`, and `ProtectHome` means it
must not live under `/home`.

`packaging/server/systemd/limedl-server.env.example` documents the one variable
the daemon consumes, `LIMEDL_RPC_SECRET`. Because a non-loopback bind refuses to
start without authentication, a missing or empty env file makes the unit fail
closed rather than expose an anonymous RPC endpoint. The example is installed to
`/etc/limedl-server.env` with mode `0600` and owned by root.

Evidence: `repo://packaging/server/systemd/limedl-server.service#L1-L38`,
`repo://packaging/server/systemd/limedl-server.env.example#L1-L13`,
`repo://crates/limedl-core/src/aria2_rpc/server.rs#L51-L59`.

## Container

### Entrypoint and privilege drop

`packaging/server/docker-entrypoint.sh` follows the common NAS container
convention. When the container starts as root it creates `/var/lib/limedl` and
`/downloads`, `chown`s them to `PUID:PGID` (default `1000:1000`), and `exec`s the
daemon through `su-exec` so the running process is unprivileged. When it starts
as a non-root user it just `exec`s the daemon, which lets `docker run --user`
work without extra setup. The ownership fix is best-effort (`|| true`) so a
read-only mount does not abort startup.

The container command already passes `--rpc-listen 0.0.0.0`, so
`LIMEDL_RPC_SECRET` is required exactly as it is for the systemd unit.

Evidence: `repo://packaging/server/docker-entrypoint.sh#L1-L19`.

### Two Dockerfiles

- `packaging/server/Dockerfile` builds from source. Its builder stage is
  `rust:1-alpine` with `build-base cmake perl linux-headers` (the C toolchain
  `aws-lc-sys` pulls in). It reads BuildKit's `TARGETARCH`, maps it to the
  matching `*-unknown-linux-musl` triple and passes it to `cargo build
  --target`, which is what makes `.cargo/config.toml`'s per-target rustflags
  (including the `--cfg reqwest_unstable` reqwest's HTTP/3 feature needs) apply.
  The runtime stage is `alpine:3.20` plus `ca-certificates`, `su-exec` and
  `tzdata`. No font fetch is needed: `-p limedl-server` never compiles
  `limedl-native`, so the embedded MiSans font is irrelevant.
- `packaging/server/Dockerfile.release` is CI-only. It takes a path to a
  prebuilt static binary via `--build-arg BINARY=...` and layers the same
  runtime on top. The release pipeline uses it so a multi-arch image never has to
  run the Rust and aws-lc C toolchain under emulation.

Evidence: `repo://packaging/server/Dockerfile#L1-L42`,
`repo://packaging/server/Dockerfile.release#L1-L27`.

### Compose

`packaging/server/docker-compose.yml` is a copy-paste example: the published
`ghcr.io/zkz098/limedl-server:latest` image, `LIMEDL_RPC_SECRET`, `PUID`/`PGID`,
a `6800:6800` port mapping and bind mounts for `/var/lib/limedl` and
`/downloads`. A commented `build:` block switches it to the from-source
Dockerfile. Publishing only to `127.0.0.1` (or fronting it with a reverse proxy)
is the safer alternative when the host is not trusted.

Evidence: `repo://packaging/server/docker-compose.yml#L1-L25`.

## Where these assets come from

The `build-server` release job copies `limedl-server.service` and
`limedl-server.env.example` into each arch tarball, so the archive is a complete
install kit. The `server-image` job extracts the same static binaries and
assembles the multi-arch GHCR image with `Dockerfile.release`. Neither the tarball
nor the image is signed or added to the desktop self-update manifest — the daemon
has no self-updater.

`.dockerignore` at the repository root keeps the build context to source plus
`packaging/` and `dist/`, excluding `target/`, `.git`, `node_modules`,
`openwiki/`, `website/` and the 20 MB font. `dist/` is deliberately retained
because `Dockerfile.release` reads the binary from it.

Evidence: `repo://.github/workflows/release.yml#L517-L592`,
`repo://.github/workflows/release.yml#L608-L693`,
`repo://.dockerignore#L1-L10`.

Related pages: [Headless Server Daemon](../integrations/headless-server-daemon.md),
[Build, Release and CI Operations](build-release-and-ci.md).
