# Desktop build dependencies and Linux packaging

Why the `limedl-native` dependency graph looks the way it does, and how the Linux
artifacts are produced. UI architecture is in
[`openwiki/desktop/native-ui-architecture.md`](../openwiki/desktop/native-ui-architecture.md).

## `rfd` dialog backend is `xdg-portal`

`rfd` allows exactly one of `gtk3` and `xdg-portal`; enabling both makes its
`build.rs` panic (`You can't enable both`). The current pin is:

```toml
rfd = { version = "0.16", default-features = false, features = ["xdg-portal", "tokio"] }
```

- Historical reason: the deleted `src-tauri` pulled `tauri-plugin-dialog`, which
  default-enabled `rfd/gtk3`; Cargo unifies features per package, so this crate
  had to follow the same backend. **That constraint is gone**; `xdg-portal` is now
  an intentional choice.
- Why it: it removes the **build-time** GTK dependency. `gtk3` needs
  `libgtk-3-dev` + `pkg-config` (`gtk-sys` uses pkg-config); `xdg-portal` talks to
  `xdg-desktop-portal` over D-Bus (ashpd/zbus, pure Rust) and gives the correct
  native file chooser on Wayland and inside Flatpak/Snap sandboxes.
- Runtime: it needs a running portal service (GNOME/KDE/XFCE ship
  `xdg-desktop-portal-*-gtk` by default). A bare X11 session with no portal gets a
  portal error — a GTK-free build cannot fall back to a GTK dialog.
- **No extra runtime feature is needed**: `rfd` drives ashpd through
  `pollster::block_on`, and ashpd's default feature is `tokio`. `tokio` is
  required alongside `xdg-portal` because `rfd`'s `build.rs` panics without one of
  `tokio`/`async-std`.
- These features only apply on Linux (the gtk/ashpd dependencies are declared
  under the Linux `target.'cfg(...)'.dependencies`); Windows/macOS graphs and
  behaviour are unchanged.

## Tray and GTK removal

The tray no longer needs GTK either: on Linux `tray-icon` uses the `ksni` backend
(a pure-Rust StatusNotifierItem; ksni + zbus over the session D-Bus), so `gtk`,
`gtk-sys`, `libappindicator` and `glib 0.18` all left the graph. No
`libgtk-3-dev`/`libappindicator3-dev` at build time and no
`libgtk-3.0`/`libayatana-appindicator3-1` at runtime. The menu code is unchanged —
the ksni backend reads the same `muda::Menu` through its snapshot, and the
snapshot's activate closures still emit `muda::MenuEvent` (details in
`crates/limedl-native/Cargo.toml`).

Removing GTK **still requires `libfontconfig1-dev`**: Slint's font stack
(`fontdb` → `yeslogic-fontconfig-sys`) probes fontconfig with `pkg-config` in its
build script, and the linked binary dynamically depends on
`libfontconfig.so.1`. GTK used to provide this transitively, so the CI Linux job
and the release Linux leg must install it explicitly or the native build panics in
fontconfig's build script.

Removed upstream warning: the `gtk 0.18` → `glib 0.18.x` unsoundness advisory
(GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429) disappeared with the ksni switch — no
`gtk`/`libappindicator` in the graph, so nothing to dismiss. Reverting to the
`libappindicator` backend brings it back (the last `libappindicator 0.9.0` hard-
depends on `glib ^0.18`).

## Linux desktop release

- Target: `x86_64-unknown-linux-gnu.2.17` via `cargo zigbuild`
  (`.cargo/config.toml` sets `x86-64-v3`, matching the Windows desktop, so a
  2013+ CPU is required). gnu rather than musl because Slint links system
  libraries — `libfontconfig.so.1` is a direct `DT_NEEDED`, and X11/Wayland/GL are
  `dlopen`ed at runtime — and a static musl binary has no dynamic loader to open
  the distro's GPU drivers (a musl build also fails earlier, in
  `yeslogic-fontconfig-sys`, for lack of a musl sysroot). glibc-version targeting
  keeps those dynamic dependencies while making the linker resolve the binary's
  *own* libc references against glibc 2.17 (Rust's minimum for the gnu target),
  so the floor drops from the `ubuntu-latest` host's 2.39 to 2.17 — Debian 10 /
  Ubuntu 18.04 / CentOS 7 and newer. `scripts/check-glibc-floor.sh` asserts the
  bound in the release job and fails if a dependency raises it.
- Artifacts:
  - `limedl-native-v{V}-linux-x86_64-portable.tar.gz` — `scripts/package-linux.sh`,
    one top-level `limedl-native/` directory (binary + README), unpack and run.
  - `limedl-native-v{V}-linux-x86_64.deb` — `scripts/package-deb.sh`, standard
    Debian/Ubuntu package with `/usr/bin/limedl-native`, a `.desktop` file and
    multi-size icons.
  - `limedl-native-v{V}-linux-x86_64.AppImage` — `scripts/package-appimage.sh`,
    a single cross-distro file embedding `AppRun`, the desktop entry and icons.
- Runtime tray dependency: the tray is a StatusNotifierItem on the session D-Bus
  and needs a D-Bus session plus a StatusNotifier host (GNOME needs the AppIndicator
  extension; KDE/XFCE/waybar ship one). With no host, ksni does not error and keeps
  waiting for one; only a failed D-Bus connection makes `TrayIconBuilder::build()`
  fail — the tray is the only persistent UI (close-to-tray, `--hidden` autostart
  both live on it), so failure is fatal and `tray_init_failure_message` explains
  how to check the host and D-Bus.
- Autostart: `~/.config/autostart/limedl-native.desktop` (XDG), with `--hidden`.
- Single instance: loopback TCP (`open:`/`show` protocol); opening from a file
  manager uses `xdg-open`.
- Sleep inhibition is still a no-op (`power.rs` does nothing off Windows), so
  Linux/macOS do not prevent system sleep.
