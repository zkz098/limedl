# Troubleshooting

> Known warnings and issues that don't need fixing. New issues with a non-zero exit code are still real problems.

---

No open items. Everything this guide used to document came from the Tauri desktop
shell (`src-tauri/`), which has been removed:

- **`LNK4078` "multiple `.rsrc` sections"** — came from `src-tauri/build.rs` embedding a
  ComCtl32 v6 manifest while `tauri_build::build()` embedded one through `tauri-winres`.
  `crates/limedl-native/build.rs` only calls `winres` for icon/version metadata, so the
  duplicate section cannot occur. There is no custom manifest code to preserve.
- **`quick-xml` RUSTSEC-2026-0194 / RUSTSEC-2026-0195** — these were accepted advisories for
  `quick-xml 0.39.x`, pinned by `wayland-scanner 0.31.10` in the Slint/winit Linux
  dependency tree. The tree now resolves `wayland-scanner 0.31.11` → `quick-xml 0.41.0`, which is
  patched (`patched = [">= 0.41.0"]`), so `deny.toml` no longer ignores them and CI runs a
  plain `cargo audit`.
- **`winreg` `multiple-versions` warning** — came from `auto-launch` (via
  `tauri-plugin-autostart`) and `embed-resource` (via `tauri-winres`). `winreg` now appears
  exactly once (`0.52.0`, used by `crates/limedl-native/src/autostart.rs`), so `cargo deny`
  reports no duplicate.
