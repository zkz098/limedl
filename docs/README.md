# limedl operational docs

Architecture and mechanism documentation lives in the loredex wiki
(`.wiki/pages/`); start at [`.wiki/pages/quickstart.md`](../.wiki/pages/quickstart.md).

This directory holds hand-maintained **runbooks** — procedures, operational
contracts and known-issue notes that do not fit the generated wiki (which is
refreshed by a scheduled workflow and must not be hand-edited).

| Runbook | Use it when |
| --- | --- |
| [ci-operations.md](ci-operations.md) | Adding or renaming a CI job, touching cache / RUSTFLAGS, or debugging build-time |
| [update-signing-key.md](update-signing-key.md) | Rotating `sigilup` self-update signing keys (Ed25519 + Post-Quantum ML-DSA-65), or investigating update-signature failures |
| [troubleshooting.md](troubleshooting.md) | A download gets `403` from a mirror, or a warning looks like a regression |
| [desktop-build-and-packaging.md](desktop-build-and-packaging.md) | Building or packaging the desktop client on Linux; rfd / tray / GTK / fontconfig questions |
| [manual-smoke-testing.md](manual-smoke-testing.md) | Running the desktop app by hand or driving it with the MCP server |
| [server-daemon.md](server-daemon.md) | Deploying the headless `limedl-server` daemon (NAS / 软路由), reverse-proxy TLS, systemd |
| [i18n.md](i18n.md) | Adding or changing a user-visible string (Slint `@tr` or Rust `i18n::tr`), or regenerating the Rust translation catalog |
| [aria2-interop-testing.md](aria2-interop-testing.md) | Changing the aria2 RPC surface, adding a client-shape fixture, or running the aria2 oracle |
| [test-regression-notes.md](test-regression-notes.md) | Adding BT / logging / settings-form tests; regression history not yet migrated to the wiki |
