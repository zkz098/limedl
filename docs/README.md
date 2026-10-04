# limedl operational docs

Architecture and mechanism documentation lives in the generated OpenWiki
(`openwiki/`); start at [`openwiki/quickstart.md`](../openwiki/quickstart.md).

This directory holds hand-maintained **runbooks** — procedures, operational
contracts and known-issue notes that do not fit the generated wiki (which is
refreshed by a scheduled workflow and must not be hand-edited).

| Runbook | Use it when |
| --- | --- |
| [ci-operations.md](ci-operations.md) | Adding or renaming a CI job, touching cache / RUSTFLAGS, or debugging build-time |
| [update-signing-key.md](update-signing-key.md) | Rotating the self-update minisign key, or investigating update-signature failures |
| [troubleshooting.md](troubleshooting.md) | A download gets `403` from a mirror, or a warning looks like a regression |
| [desktop-build-and-packaging.md](desktop-build-and-packaging.md) | Building or packaging the desktop client on Linux; rfd / tray / GTK / fontconfig questions |
| [manual-smoke-testing.md](manual-smoke-testing.md) | Running the desktop app by hand or driving it with the MCP server |
| [server-daemon.md](server-daemon.md) | Deploying the headless `limedl-server` daemon (NAS / 软路由), reverse-proxy TLS, systemd |
| [engine-dev-notes.md](engine-dev-notes.md) | Adding a checksum algorithm, or touching async file I/O |
| [aria2-interop-testing.md](aria2-interop-testing.md) | Changing the aria2 RPC surface, adding a client-shape fixture, or running the aria2 oracle |
| [test-regression-notes.md](test-regression-notes.md) | Adding UI / BT / logging tests; looking for regressions the suite already caught |
