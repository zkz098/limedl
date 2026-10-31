---
id: operations/server-deployment-and-packaging
title: Server Deployment and Packaging
status: verified
generated_at: 2026-10-31T14:48:10Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: .github/workflows/release.yml
  lines:
  - 600
  - 612
  blob: 8ffb6ce1ff3270e1597c5c5376f1b2bca483ded4
  snippet_sha1: 1c92d66b78b05d9fd11a8c34e3c70fd3020b61cf
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 7
  - 8
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: 11f84d443aad916e4975dd7a14fa082f09a7ac2a
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 9
  - 12
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: fcc7c0bb45a92dbbdc2b4e66e6115c0ad0956d47
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 14
  - 16
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: bc1a38e4bf0a37e3c634d45ea188e01cc07a25f1
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 20
  - 25
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: bce17258825895884eddcd09adef118420f96fbe
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 31
  - 36
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: a1cf4fbcdae5b1dcaf32b6db1ed1a04d2ae6658e
- path: packaging/server/systemd/limedl-server.service
  lines:
  - 27
  - 30
  blob: bae5c92b45aa676c4334af7133a29bc3addcaa74
  snippet_sha1: e039496de0d07d4a0dacd28bc6703229a878e45a
- path: packaging/server/systemd/limedl-server.env.example
  lines:
  - 1
  - 14
  blob: 791b740436b8399206d4a6c789768ca0d11ecd37
  snippet_sha1: 4748ded9be6b9f01092996dbb75949735af85a18
- path: crates/limedl-rpc/src/server.rs
  lines:
  - 55
  - 72
  blob: 90bfb55abd2fa791e2edec0a89ed4afbcefc9c94
  snippet_sha1: 3618c1042d432ebfcdeca381e227c1a39a60a4d0
- path: crates/limedl-rpc/src/bind.rs
  lines:
  - 14
  - 28
  blob: c2525356e5129848d753a132fc7e36b75c77c109
  snippet_sha1: f78cc425fde9063449984fb25cc27d6c31145cbb
- path: packaging/server/docker-entrypoint.sh
  lines:
  - 1
  - 20
  blob: 6583b6ba88fdc527d0544256a3d69674d6855485
  snippet_sha1: 08dd476c11cdb1dd29f49704a9c3c588239217b5
- path: packaging/server/Dockerfile
  lines:
  - 42
  - 43
  blob: 9e36083594eb494f130c4d2c15ec45141cae5d50
  snippet_sha1: e16b576cab264aafc20841246a34a9e394c576ff
- path: docs/server-daemon.md
  lines:
  - 227
  - 231
  blob: 357dd5c95fe6897a29928325b9e659f072233141
  snippet_sha1: 555d9721a72a740d95fe002a75e304f731e40434
- path: packaging/server/Dockerfile
  lines:
  - 12
  - 18
  blob: 9e36083594eb494f130c4d2c15ec45141cae5d50
  snippet_sha1: 0682dea6bccafdce66b840c150571f623e85a74b
- path: .github/workflows/release.yml
  lines:
  - 540
  - 543
  blob: 8ffb6ce1ff3270e1597c5c5376f1b2bca483ded4
  snippet_sha1: 60531c950a658b99170650e737f6721cf05bea6e
- path: packaging/server/Dockerfile
  lines:
  - 13
  - 29
  blob: 9e36083594eb494f130c4d2c15ec45141cae5d50
  snippet_sha1: f0cf6edb2722f5c1497a04ef44f38ca814f8515f
- path: packaging/server/Dockerfile
  lines:
  - 31
  - 32
  blob: 9e36083594eb494f130c4d2c15ec45141cae5d50
  snippet_sha1: 91155deb65d532ba26c092f5ce546644ba7e7800
- path: packaging/server/Dockerfile
  lines:
  - 7
  - 10
  blob: 9e36083594eb494f130c4d2c15ec45141cae5d50
  snippet_sha1: 975f55ec7a4dde92f21b2b68d4c912df353b595d
- path: packaging/server/Dockerfile.release
  lines:
  - 1
  - 22
  blob: 925600eb7c8863cdb4701379c12d6cbc0ef3fa57
  snippet_sha1: db133b4de2657793f12397473ec18fc8bcd2644e
- path: packaging/server/docker-compose.yml
  lines:
  - 1
  - 26
  blob: b1b4f5c6431fd3b769d6a98ef8e8e12bcb78db23
  snippet_sha1: efaa8187574d726e77f8171851726819b3d04509
- path: docs/server-daemon.md
  lines:
  - 199
  - 201
  blob: 357dd5c95fe6897a29928325b9e659f072233141
  snippet_sha1: 1c51386f54a9ecec20ba26251b68ec1bccf66264
- path: .github/workflows/release.yml
  lines:
  - 659
  - 703
  blob: 8ffb6ce1ff3270e1597c5c5376f1b2bca483ded4
  snippet_sha1: d21924cae2de0f8292866fe12329f719e5f4e42b
- path: .github/workflows/release.yml
  lines:
  - 548
  - 549
  blob: 8ffb6ce1ff3270e1597c5c5376f1b2bca483ded4
  snippet_sha1: 467637f8ea50c73dd57ec7bb565777bed3324c0b
- path: docs/server-daemon.md
  lines:
  - 24
  - 26
  blob: 357dd5c95fe6897a29928325b9e659f072233141
  snippet_sha1: 23a7aaefc4cf32c0b73e30b95ec2d003bddc2ff8
- path: .dockerignore
  lines:
  - 1
  - 11
  blob: 80a56e502bd318bff63b980f8c06f12a59286e91
  snippet_sha1: d3b0806889aa236bc63769d728a13b2090d25336
type: operations
---
# Server Deployment and Packaging

The headless `limedl-server` daemon is deployed in one of two ways: as a native
service managed by systemd, or as a container. Both are driven by files
committed under `packaging/server/`, and the same assets ship in the release
tarball so a NAS install is one download
[[cite:.github/workflows/release.yml:L600-L612]]. Engine/CLI behaviour is on
[[wiki:integrations/headless-server-daemon]]; the step-by-step install narrative
is `docs/server-daemon.md`.

## systemd

`packaging/server/systemd/limedl-server.service` is a `Type=simple` unit
[[cite:packaging/server/systemd/limedl-server.service:L7-L8]] that:

- runs as a dedicated `limedl` user/group — the account is created by the
  operator, not the unit [[cite:packaging/server/systemd/limedl-server.service:L9-L12]];
- reads secrets from `/etc/limedl-server.env` through `EnvironmentFile`, so the
  token is not visible in the unit body or the process list
  [[cite:packaging/server/systemd/limedl-server.service:L14-L16]];
- starts `limedl-server --rpc-listen 0.0.0.0 --data-dir /var/lib/limedl
  --download-dir /srv/downloads` and restarts on failure
  [[cite:packaging/server/systemd/limedl-server.service:L20-L25]].

It applies `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome=true`,
`PrivateTmp=true`, `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX` and a
`ReadWritePaths` allow-list of `/var/lib/limedl` and `/srv/downloads`
[[cite:packaging/server/systemd/limedl-server.service:L31-L36]]. Those two paths
are the whole writable surface the daemon needs: `ReadWritePaths` must list
every path the daemon writes — the data directory, the download directory, and
the BT output directory, which lives under `<data-dir>/downloads/bt_files` by
default — and `ProtectHome=true` means a custom `--download-dir` must not live
under `/home` [[cite:packaging/server/systemd/limedl-server.service:L27-L30]].

`packaging/server/systemd/limedl-server.env.example` documents the main
variable the daemon consumes, `LIMEDL_RPC_SECRET`, and is installed to
`/etc/limedl-server.env` with mode `0600` and owner root
[[cite:packaging/server/systemd/limedl-server.env.example:L1-L14]]. Because a
non-loopback bind refuses to start without authentication, a missing or empty
env file makes the unit fail closed rather than expose an anonymous RPC endpoint
[[cite:crates/limedl-rpc/src/server.rs:L55-L72]],
[[cite:crates/limedl-rpc/src/bind.rs:L14-L28]].

## Container

### Entrypoint and privilege drop

`packaging/server/docker-entrypoint.sh` follows the common NAS container
convention. When the container starts as root it creates `/var/lib/limedl` and
`/downloads`, `chown`s them to `PUID:PGID` (default `1000:1000`), and `exec`s
the daemon through `su-exec` so the running process is unprivileged. When it
starts as a non-root user it just `exec`s the daemon, which lets
`docker run --user` work without extra setup. The ownership fix is best-effort
(`|| true`) so a read-only mount does not abort startup
[[cite:packaging/server/docker-entrypoint.sh:L1-L20]].

The container command already passes `--rpc-listen 0.0.0.0`, so
`LIMEDL_RPC_SECRET` is required exactly as it is for the systemd unit
[[cite:packaging/server/Dockerfile:L42-L43]].

Leaving the container running says nothing about the *host's* power state: the
daemon never inhibits sleep, so a host that suspends on idle will stall the
queue whatever the container is doing. Keeping the machine awake is the host
operator's job — pause the queue or disable idle suspend on the NAS itself
[[cite:docs/server-daemon.md:L227-L231]].

### Two Dockerfiles

- `packaging/server/Dockerfile` builds from source. Its builder stage is
  `rust:1-alpine` with `build-base cmake linux-headers perl` (the C toolchain
  `aws-lc-sys` pulls into any musl build)
  [[cite:packaging/server/Dockerfile:L12-L18]],
  [[cite:.github/workflows/release.yml:L540-L543]]. It reads BuildKit's
  `TARGETARCH`, maps it to the matching `*-unknown-linux-musl` triple and passes
  it to `cargo build --target`, which is what makes `.cargo/config.toml`'s
  per-target rustflags (including the `--cfg reqwest_unstable` reqwest's HTTP/3
  feature needs) apply [[cite:packaging/server/Dockerfile:L13-L29]]. The runtime
  stage is `alpine:3.20` plus `ca-certificates`, `su-exec` and `tzdata`
  [[cite:packaging/server/Dockerfile:L31-L32]]. No font fetch is needed:
  `limedl-server` does not depend on Slint, the tray or the embedded MiSans font
  [[cite:packaging/server/Dockerfile:L7-L10]].
- `packaging/server/Dockerfile.release` is CI-only. It takes a path to a
  prebuilt static binary via `--build-arg BINARY=...` and layers the same
  runtime on top. The release pipeline uses it so a multi-arch image never has
  to run the Rust and aws-lc C toolchain under emulation
  [[cite:packaging/server/Dockerfile.release:L1-L22]].

### Compose

`packaging/server/docker-compose.yml` is a copy-paste example: the published
`ghcr.io/zkz098/limedl-server:latest` image, `LIMEDL_RPC_SECRET`, `PUID`/`PGID`,
a `6800:6800` port mapping and bind mounts for `/var/lib/limedl` and
`/downloads`. A commented `build:` block switches it to the from-source
Dockerfile [[cite:packaging/server/docker-compose.yml:L1-L26]]. Publishing only
to `127.0.0.1` (or fronting it with a reverse proxy) is the safer alternative
when the host is not trusted [[cite:docs/server-daemon.md:L199-L201]].

## Where these assets come from

The `build-server` release job copies `limedl-server.service` and
`limedl-server.env.example` into each arch tarball, so the archive is a complete
install kit [[cite:.github/workflows/release.yml:L600-L612]]. The `server-image`
job extracts the same static binaries and assembles the multi-arch GHCR image
with `Dockerfile.release` [[cite:.github/workflows/release.yml:L659-L703]].
Neither the tarball nor the image is signed or added to the desktop self-update
manifest — they are plain release assets
[[cite:.github/workflows/release.yml:L548-L549]],
[[cite:docs/server-daemon.md:L24-L26]].

`.dockerignore` at the repository root keeps the build context to source plus
`packaging/` and `dist/`, excluding `target/`, `.git`, `node_modules`,
`openwiki/`, `website/` and the 20 MB font. `dist/` is deliberately retained
because `Dockerfile.release` reads the binary from it
[[cite:.dockerignore:L1-L11]].

Related pages: [[wiki:integrations/headless-server-daemon]],
[[wiki:operations/build-release-and-ci]].
