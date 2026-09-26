# Troubleshooting

> Known warnings and issues that don't need fixing. New issues with a non-zero exit code are still real problems.

---

## 镜像站反滥用 403（例如清华 TUNA）

**症状**：浏览器能下载同一 URL，limedl 返回 `403 Forbidden`，错误信息为
`http status 403 Forbidden (server anti-abuse check rejected this client; update the default User-Agent in Settings or try another mirror)`。

**原因**：TUNA（`mirrors.tuna.tsinghua.edu.cn`）边缘按 UA 分类客户端：UA 声称是浏览器但版本过期或尚未发布时，被判为“非常用软件”（伪装浏览器）而拒绝。实测（2026-09，当时稳定版 Chrome 154）：

| UA | 结果 |
| --- | --- |
| `Chrome/124`（旧内置默认） | GET 403；HEAD 200（HEAD 被豁免，所以探测阶段看似正常） |
| `Chrome/137`–`Chrome/141` | 206 成功 |
| `Chrome/143`、`Chrome/145` | 403（尚未发布的版本同样被拒） |
| `curl/8.7.1`、`aria2/1.37.0`、`limedl/0.3.13` | 206（工具型 UA 走另一条通道） |
| IPv4 出口 | 该网段被整体关注时所有 UA 均 403；IPv6 正常 |

**处理**：

1. 升级内置 UA：`crates/limedl-core/src/types/settings.rs::default_http_user_agent()` 改为当前稳定版 Chrome 大版本，并同步 UI 文案与 i18n（见 `subsystem-http-client-factory.md`）。
2. 已存在的 `settings.json` 不会自动跟随（保留用户自定义值），可在设置里清空 Default User-Agent 或手填新的浏览器 UA。
3. UA 已更新仍 403 时，用 `curl -4` / `curl -6` 对比：IPv4 网段被 TUNA 整体关注时需要换网络或走 IPv6。
4. 这是服务端策略而非 limedl 的 Referer 问题；代码已识别反滥用页并跳过无意义的 Referer 探测（见 `subsystem-download-manager.md`）。

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
