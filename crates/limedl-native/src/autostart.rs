#![allow(dead_code)]
// Native autostart integration — OS-specific implementation.
// Windows: HKCU\Software\Microsoft\Windows\CurrentVersion\Run (registry)
// Linux:   ~/.config/autostart/limedl-native.desktop (XDG)
// macOS:   ~/Library/LaunchAgents/com.zkz20.limedl.plist

fn current_exe_string() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[cfg(windows)]
mod windows_impl {
    use super::current_exe_string;
    use anyhow::Context;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "limedl-native";

    pub fn is_enabled() -> bool {
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(VALUE_NAME))
            .is_ok()
    }

    pub fn enable() -> anyhow::Result<()> {
        let exe = current_exe_string().context("failed to get current exe path")?;
        // Quote path if it contains spaces
        let val = if exe.contains(' ') {
            format!("\"{exe}\"")
        } else {
            exe
        };
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_WRITE)
            .context("open Run key")?
            .set_value(VALUE_NAME, &val)
            .context("set Run value")?;
        Ok(())
    }

    pub fn disable() -> anyhow::Result<()> {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_WRITE)
            .context("open Run key")?;
        match key.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(not(windows))]
mod windows_impl {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn enable() -> anyhow::Result<()> {
        anyhow::bail!("autostart enable only implemented on Windows")
    }
    pub fn disable() -> anyhow::Result<()> {
        anyhow::bail!("autostart disable only implemented on Windows")
    }
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::current_exe_string;
    use std::path::PathBuf;

    fn autostart_file() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("autostart").join("limedl-native.desktop"))
    }

    pub fn is_enabled() -> bool {
        autostart_file().is_some_and(|p| p.exists())
    }

    pub fn enable() -> anyhow::Result<()> {
        let exe = current_exe_string().ok_or_else(|| anyhow::anyhow!("no exe"))?;
        let path = autostart_file().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=limedl-native\nExec={exe}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n"
        );
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn disable() -> anyhow::Result<()> {
        if let Some(p) = autostart_file() {
            if p.exists() {
                std::fs::remove_file(p)?;
            }
        }
        Ok(())
    }
}

#[cfg(all(not(target_os = "linux"), not(windows)))]
mod linux_impl {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn enable() -> anyhow::Result<()> {
        Ok(())
    }
    pub fn disable() -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::current_exe_string;
    use std::path::PathBuf;

    fn plist_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/LaunchAgents/com.zkz20.limedl.plist"))
    }

    pub fn is_enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }

    pub fn enable() -> anyhow::Result<()> {
        let exe = current_exe_string().ok_or_else(|| anyhow::anyhow!("no exe"))?;
        let path = plist_path().ok_or_else(|| anyhow::anyhow!("no home"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.zkz20.limedl</string>
    <key>ProgramArguments</key><array><string>{exe}</string></array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
"#
        );
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn disable() -> anyhow::Result<()> {
        if let Some(p) = plist_path() {
            if p.exists() {
                std::fs::remove_file(p)?;
            }
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod macos_impl {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn enable() -> anyhow::Result<()> {
        Ok(())
    }
    pub fn disable() -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        windows_impl::is_enabled()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::is_enabled()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::is_enabled()
    }
    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        false
    }
}

pub fn enable() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        windows_impl::enable()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::enable()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::enable()
    }
    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        Ok(())
    }
}

pub fn disable() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        windows_impl::disable()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::disable()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::disable()
    }
    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        Ok(())
    }
}

pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    if enabled {
        enable()
    } else {
        disable()
    }
}

/// Sync the persisted `settings.autostart` boolean with the actual OS registration.
/// Called at startup to ensure the file value and OS state are consistent, and
/// after every successful settings save.
pub fn sync_from_settings(autostart_flag: bool) {
    let os_enabled = is_enabled();
    if autostart_flag != os_enabled {
        let res = set_enabled(autostart_flag);
        if let Err(e) = res {
            tracing::warn!("autostart sync failed (want={autostart_flag}, os={os_enabled}): {e:#}");
        } else {
            tracing::info!("autostart sync: set OS autostart to {autostart_flag}");
        }
    }
}
