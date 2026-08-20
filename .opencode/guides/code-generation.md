# Code Generation

> Auto-generated TypeScript files from Rust source. Never edit generated files manually.

## When to regenerate

After any of:

- Adding/modifying a Rust struct/enum with `#[derive(TS)]` / `#[cfg_attr(feature = "ts", derive(TS))]`
- Adding a new entry to `WS_COMMANDS` or `WS_EVENTS` in `ws_manifest.rs`
- Adding a new `DownloadEvent` variant

**Command** (Windows: init MSVC first):

```powershell
$env:TS_RS_EXPORT_DIR="."
cargo test --manifest-path crates/limedl-core/Cargo.toml --features ts -- export_typescript_bindings
```

Output:

- `src/types/generated/types.ts` — Rust type definitions (ts-rs)
- `src/types/generated/settings-default.ts` — `AppSettings::default()` as TS (`DEFAULT_APP_SETTINGS`; Rust is the single source of truth, consumed via `src/lib/app-settings-defaults.ts` re-export)
- `src/lib/ws/generated/ws-commands.ts` — WS command manifest
- `src/lib/ws/generated/ws-events.ts` — WS event manifest

CI verifies freshness: `git diff --exit-code src/types/generated/ src/lib/ws/generated/`

---

## TS type generation (ts-rs)

Source of truth: Rust structs/enums in `crates/limedl-core/src/`.

- `ts-rs` is an **optional** dependency behind the `ts` feature.
- Types use `#[cfg_attr(feature = "ts", derive(TS))]` and export paths.
- The export test at `crates/limedl-core/src/tests/ts_export.rs` triggers export.
- Frontend re-export files (`src/types/settings.ts`, `download.ts`, `cdn.ts`) add only pure-frontend types.

Rules:

- Never manually edit TypeScript types matching Rust structs — edit Rust and regenerate.
- Generated files must be committed alongside Rust changes.

---

## WebSocket command manifest

Source of truth: `crates/limedl-core/src/ws_manifest.rs`

- `WsCommandSpec { tauri_name, rpc_method, param_transform }`
- `ParamTransform` enum: `Identity`, `Rename`, `UnwrapField`
- `WS_COMMANDS` array lists all commands (currently 33)

### Adding a new WS command

1. Add a `WsCommandSpec` entry to `WS_COMMANDS` in `ws_manifest.rs`
2. Add handler branch in `crates/limedl-server/src/rpc.rs` dispatch (and sub-handlers for grouped dispatch)
3. Regenerate (`cargo test --features ts -- export_typescript_bindings`)
4. If non-Identity transform, add variant to `ParamTransform` and handle in `ws-invoke.ts`'s `applyTransform`
5. Commit ws_manifest.rs, rpc.rs, and generated `ws-commands.ts`

⚠️ Compile-time test `all_rpc_methods_have_dispatch_arms` verifies every `tauri_name` appears in rpc.rs.

---

## WebSocket event manifest

Source of truth: `crates/limedl-core/src/ws_manifest.rs` (same file as commands)

- `WsEventSpec { ws_type, tauri_event_name }`
- `WS_EVENTS` array — 7 entries covering all `DownloadEvent` variants
- Generated `ws-events.ts` exports `EVENT_TYPE_MAP` (ws_type → tauri_event_name)

### Adding a new DownloadEvent variant

1. Add variant to `DownloadEvent` in `crates/limedl-core/src/event_bus/mod.rs`
2. Add `WsEventSpec` entry to `WS_EVENTS` in `ws_manifest.rs`
3. Add emit branch in `src-tauri/src/lib.rs` Tauri adapter
4. Add notification handler in `crates/limedl-server/src/rpc.rs` RPC adapter
5. Regenerate
6. Commit all 5 files

⚠️ Compile-time tests verify ws_type/tauri_event_name appear in both adapters.
Exception: `aria2Notification` in Tauri uses dynamic event_name (not checked).
