# Manual smoke testing

## Desktop app

```powershell
$env:LIMEDL_DATA_DIR = "$env:TEMP\limedl-smoke"
cargo run -p limedl-native
```

Logs are at `<data_dir>\downloads\logs\limedl.log` (level controlled by
`settings.logging.level`; set it to `info` when debugging).

## Driving a real window with the MCP server

To let an agent or script open the real window and inspect/click it, use the MCP
server instead of a bare `cargo run`. Full prerequisites are in
[`openwiki/testing/slint-ui-testing.md`](../openwiki/testing/slint-ui-testing.md);
the short version:

```cmd
set "SLINT_EMIT_DEBUG_INFO=1"
set "SLINT_MCP_PORT=8080"
cargo run -p limedl-native --features slint/mcp
```

- **Element introspection needs compiler-embedded debug metadata.** `build.rs`
  turns it on automatically for the debug profile, so `cargo run`/`cargo test`
  need no setup; a release build needs `SLINT_EMIT_DEBUG_INFO=1` at **build** time
  (`CompilerConfiguration::new()` reads it, and `build.rs` only forces it on in
  debug without overriding an explicit release setting). Without the metadata
  every id lookup silently returns nothing and prints one warning.
- **Quit any running limedl first.** The single-instance guard makes a second
  process notify the primary and exit immediately (exit 0, no error), so the port
  never opens and it looks like the server is broken. Confirm with
  `Get-Process limedl-native`.
- **Do not pass `--hidden`.** The MCP server starts from the "first window shown"
  hook, and `--hidden` (with `setup_completed` true) never calls `show()`, so the
  server does not come up; the same applies to the MSIX logon path.
- Isolate the session with `LIMEDL_DATA_DIR=%TEMP%\limedl-mcp` so the real
  settings/database are untouched.
- No display (CI/container): add `SLINT_BACKEND=headless`. That value only exists
  when the `mcp` feature is compiled in, and Slint marks it unstable — automation
  only.
- The server binds `127.0.0.1`, has no authentication and validates the `Origin`
  header, so it is a local development tool: never expose the port.
- `slint/mcp` is part of the open-source `slint` crate. The Python `slint_testing`
  client (the "GUI Test Framework" at `testing.slint.dev`) is the **commercial**
  product and needs a licence — do not assume it is available.
