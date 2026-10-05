---
type: integration
title: Headless Server Daemon (limedl-server)
description: The GUI-free limedl-server binary — CLI and data-dir resolution, the data-directory instance lock, forced-enabled Aria2 RPC with exit_on_shutdown, SIGTERM/SIGINT shutdown, static musl packaging and the end-to-end daemon test.
tags: [server, daemon, cli, headless, nas, aria2]
sources:
  - id: openwiki-source-164e2da859b5277df81c7d94
    resource: repo://.github/workflows/ci.yml
  - id: openwiki-source-4d1d392666be6dfdd7a91a2e
    resource: repo://.github/workflows/release.yml
  - id: openwiki-source-9f796a37ea60bc20889db159
    resource: repo://crates/limedl-server/src/config.rs
  - id: openwiki-source-2d1753b77bfe7d551752205e
    resource: repo://crates/limedl-server/src/lib.rs
  - id: openwiki-source-12c6f791eb18ee845f599026
    resource: repo://crates/limedl-server/src/main.rs
  - id: openwiki-source-12b12baa98b75281cf2fd260
    resource: repo://crates/limedl-server/tests/daemon.rs
  - id: openwiki-source-7ae9f72a70d7b68c2986edb0
    resource: repo://packaging/server/docker-compose.yml
  - id: openwiki-source-7003b6883ff44b1304d949e8
    resource: repo://packaging/server/Dockerfile
  - id: openwiki-source-816a10881eb55b69eaf93236
    resource: repo://packaging/server/systemd/limedl-server.service
generated: { by: "pi", at: "2026-10-05T01:38:26.934Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T01:38:26.934Z
---

# Headless Server Daemon (`limedl-server`)

`crates/limedl-server/` is a fourth workspace member that runs the same engine as
the desktop client with no GUI and exposes the Aria2 JSON-RPC API, so AriaNg,
Motrix and aria2 scripts can drive it on a NAS, a soft router or a container. It
is the second frontend over the shared `Dispatcher`: `main` is a two-line wrapper
around the library, and the library is deliberately the testable part.

Evidence: `repo://crates/limedl-server/src/main.rs#L1-L7`,
`repo://crates/limedl-server/src/lib.rs#L1-L10`.

## Entry point and startup sequence

`main` is `#[tokio::main]` and calls `main_entry()`, which parses argv into `Cli`,
resolves a `Config`, and runs `run_with(config, shutdown_signal())`.
`run_with` takes the shutdown future as a parameter so an integration test can
start the daemon without raising a process signal.

`run_with` performs, in order:

1. create `state_dir` (`<data_dir>/downloads`);
2. install logging from `LogSettings::default()` with the CLI level, so engine
   start-up messages are captured;
3. take the exclusive data-directory instance lock;
4. `bootstrap(state_dir)` — the same call the desktop makes;
5. persist `--download-dir` through `Dispatcher::save_settings_with` when given;
6. re-apply the on-disk logging settings (the CLI level still wins);
7. clone `settings.aria2_rpc`, force the daemon's overrides, and build
   `Aria2RpcServer`;
8. spawn `serve` and select on it, the signal future and the shutdown `Notify`;
9. call `core.registry.shutdown_all()`.

Evidence: `repo://crates/limedl-server/src/lib.rs#L29-L143`.

## Configuration resolution

| Flag | Effect |
| --- | --- |
| `--data-dir <PATH>` | Base directory; highest precedence. |
| `--rpc-listen <ADDR>` | Bind address override (`0.0.0.0` for LAN). |
| `--rpc-port <PORT>` | Port override. |
| `--rpc-secret <TOKEN>` | Forces shared-secret auth; falls back to `$LIMEDL_RPC_SECRET`. |
| `--rpc-allow-origin-all` | Sets `allow_any_origin`. |
| `--rpc-allow-origin <ORIGIN>` | Appends a specific CORS origin (repeatable). |
| `--download-dir <PATH>` | Must be absolute; persisted. |
| `--log-level <LEVEL>` | `trace`/`debug`/`info`/`warn`/`error`, mapped from a local `LogLevelArg` enum because `limedl-core` does not depend on `clap`. |

Every flag overrides `settings.json` for that run only, except `--download-dir`,
which is persisted. The data directory follows the desktop's precedence:
`--data-dir`, then `$LIMEDL_DATA_DIR`, then the platform local data dir, then a
temp fallback; `state_dir` is always `<data_dir>/downloads`, matching
`SystemContext`'s rule that `settings.json` is the parent of the state directory.
The secret is read from `$LIMEDL_RPC_SECRET` when the flag is absent, which keeps
it out of the process list.

Evidence: `repo://crates/limedl-server/src/cli.rs#L25-L85`,
`repo://crates/limedl-server/src/config.rs#L19-L99`.

## The daemon's RPC overrides

The daemon differs from the desktop in exactly three RPC settings, applied in
`run_with`:

- `enabled = true` — the RPC endpoint is the daemon's only interface, so it does
  not honour `settings.json`'s disabled default;
- `exit_on_shutdown = true` — `aria2.shutdown` stops the process (aria2
  semantics), where the desktop keeps it off because the UI owns exit;
- CLI overrides for `listen_address`, `port`, `secret` (which also forces
  `Aria2AuthMode::Single`), `allow_any_origin` and additional CORS origins.

It then relies on the core server's fail-closed gate: a non-loopback
`listen_address` without authentication makes `serve` return an error, which
propagates as the daemon's startup failure rather than silently exposing an
anonymous endpoint. `Aria2RpcServer::shutdown_notify()` is cloned out before
`serve` consumes the server so the `aria2.shutdown` notification can be awaited.

Evidence: `repo://crates/limedl-server/src/lib.rs#L78-L118`,
`repo://crates/limedl-core/src/aria2_rpc/server.rs#L51-L59`.

## Instance lock

Two daemons pointed at one data directory would race on the SQLite database, the
torrent state and the RPC port. `acquire_instance_lock` creates
`<data_dir>/limedl-server.lock` and calls `File::try_lock`; a second instance
fails with an explanatory error. The lock is held for the process lifetime and
released by the OS on exit, so a crash does not leave a stale lock.

This guards against a second *daemon* only. It does not make a desktop client
safe to run against the same data directory; that is why `docs/server-daemon.md`
tells operators to stop one before starting the other.

Evidence: `repo://crates/limedl-server/src/lib.rs#L148-L165`,
`repo://crates/limedl-server/src/lib.rs#L194-L214`.

## Shutdown and signals

`run_with` spawns `serve` as a task, then `tokio::select!`s on:

- the injected shutdown future (`main_entry` passes `shutdown_signal`),
- the `shutdown_notify` handle that `aria2.shutdown` fires, and
- the `serve` task itself, so a bind error propagates instead of hanging.

Whichever wins, the daemon sends `true` on the server's `watch` channel (when a
shutdown was requested), awaits the serve task, and then calls
`registry.shutdown_all()` to flush manifests, checkpoint the WAL and release the
BT session. `shutdown_signal` is `SIGTERM`/`SIGINT` on Unix and `ctrl_c` on
non-Unix, mirroring the desktop's signal watcher but without the Slint event
loop.

Evidence: `repo://crates/limedl-server/src/lib.rs#L106-L143`,
`repo://crates/limedl-server/src/lib.rs#L166-L192`.

## Packaging and deployment

The crate has no Slint, tray or dialog dependency — only `limedl-core` with
`aria2-rpc`, `clap`, `tokio`, `anyhow` and `tracing` — so it cross-compiles to a
fully static musl binary. The release pipeline's `build-server` job builds
`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` with
`cargo zigbuild` (zig bundles the musl C toolchain `aws-lc-sys` needs, without
Docker) and uploads tarballs that also contain the systemd unit and env example.
Both targets were verified locally to produce stripped, statically-linked
binaries. `.cargo/config.toml` already carries both targets, with the x86_64
entry at `target-cpu=x86-64-v2` for older NAS CPUs.

The daemon also ships as a multi-arch `ghcr.io/zkz098/limedl-server` image that
the `server-image` job assembles from those same prebuilt binaries. The committed
deployment assets (`packaging/server/`) and their install procedure are documented
on [Server Deployment and Packaging](../operations/server-deployment-and-packaging.md),
with operational guidance (systemd, reverse-proxy TLS, security trade-offs) in
`docs/server-daemon.md`.

Evidence: `repo://crates/limedl-server/Cargo.toml#L13-L22`,
`repo://.github/workflows/release.yml#L517-L592`,
`repo://.github/workflows/release.yml#L608-L693`,
`repo://.cargo/config.toml#L28-L38`.

## Testing

Pure unit tests cover CLI parsing, `LogLevelArg` → `LogLevel` mapping, config
precedence/trimming, absolute-path validation, the loopback data-dir override and
the instance lock's exclusivity.

The end-to-end test in `tests/daemon.rs` is the one that proves the wiring: it
starts a real daemon against a temp data directory on a reserved port, polls
`aria2.getVersion` until the engine is up, asserts a missing token is rejected,
calls `aria2.shutdown`, and awaits the daemon task exiting cleanly. The readiness
poll uses a fallible `rpc_try` and treats a refused connection as "not ready
yet", because the engine bootstraps and binds the port asynchronously — only the
request after readiness may panic. It runs in the Linux `check-rust` job; the
Windows/macOS legs only compile the crate via the workspace clippy.

Evidence: `repo://crates/limedl-server/tests/daemon.rs#L24-L123`,
`repo://crates/limedl-server/src/config.rs#L119-L192`,
`repo://.github/workflows/ci.yml#L507-L514`.

Related pages: [Aria2 JSON-RPC Compatibility Server](aria2-rpc-server.md),
[Bootstrap, SystemContext and Shared Services](../architecture/bootstrap-and-services.md),
[Server Deployment and Packaging](../operations/server-deployment-and-packaging.md),
[Build, Release and CI Operations](../operations/build-release-and-ci.md).
