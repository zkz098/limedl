# Troubleshooting

> Known warnings and issues that don't need fixing. New issues with a non-zero exit code are still real problems.

---

## `LNK4078` on Windows release builds

```
resource.lib : warning LNK4078: found multiple ".rsrc" sections with different attributes (40000040)
```

**Harmless.** Caused by `build.rs` manually embedding a ComCtl32 v6 manifest for the binary target, while `tauri_build::build()` also embeds one via `tauri-winres`. The manual embedding is intentional — it ensures the manifest is present in test binaries (`cargo test --workspace`), not just the release binary. Do not remove the custom `build.rs` manifest code.

---

## `cargo audit` reports quick-xml RUSTSEC-2026-0194 / RUSTSEC-2026-0195

`cargo audit` will always report 2 high-severity advisories against `quick-xml 0.39.4`:

- **RUSTSEC-2026-0194** — `BytesStart::attributes()` O(N²) duplicate-name check → CPU DoS
- **RUSTSEC-2026-0195** — `NsReader` unbounded namespace-declaration allocation → OOM

**Status: known, accepted, no remediation possible from limedl side.**

- quick-xml enters as a **build-time** dependency of `wayland-scanner 0.31.10` (proc-macro parsing host-preinstalled Wayland protocol XML at compile time). limedl source has zero `quick_xml` / `BytesStart::attributes` / `NsReader` imports.
- wayland-scanner declares `quick-xml = "^0.39"` — `cargo update` cannot lift across the 0.39 → 0.41 major bump.
- Attack path does not apply: limedl's build parses only trusted platform Wayland protocol DTDs.
- `deny.toml` ignores both advisory IDs with rationale. CI passes `--ignore RUSTSEC-2026-0194 --ignore RUSTSEC-2026-0195`. New advisories still fail CI.
- **Revisit when** upstream publishes a release bumping quick-xml (wayland-scanner 0.31.11+ or transitive via tauri-plugin-clipboard-manager).

---

## `cargo deny` `multiple-versions` warning for `winreg`

`winreg 0.10.1` (via `auto-launch` ← `tauri-plugin-autostart`) and `winreg 0.55.0` (via `embed-resource` ← `tauri-winres` ← `tauri-build`). Two major versions are incompatible at API level but don't coexist at runtime in the same module path. Supplied upstream via Tauri's plugin/build stack. Don't add `skip = ["winreg"]` to `deny.toml`; the warning is informative.
