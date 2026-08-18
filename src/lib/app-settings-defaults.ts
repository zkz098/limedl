/**
 * Frontend fallback default for `AppSettings`.
 *
 * The value itself is generated from Rust (see
 * `src/types/generated/settings-default.ts`, produced by the ts-rs codegen
 * from `AppSettings::default()` in `crates/limedl-core/src/types.rs`), so Rust
 * is the single source of truth — no hand-maintained TS copy to drift here.
 *
 * When the Tauri backend is reachable, prefer `getAppSettings()` — the backend
 * returns the authoritative (possibly user-modified) settings.
 */
export { DEFAULT_APP_SETTINGS } from "../types/generated/settings-default";
