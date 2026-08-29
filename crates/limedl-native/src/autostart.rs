#![allow(dead_code)]
// Native autostart integration — OS-specific implementation.
// Windows portable/NSIS: HKCU\Software\Microsoft\Windows\CurrentVersion\Run (registry)
// Windows MSIX/Store:    windows.startupTask manifest extension + StartupTask API
//                        (registry Run is virtualized inside MSIX and would be lost)
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
    /// Matches `TaskId` in packaging/msix/AppxManifest.xml.
    const STARTUP_TASK_ID: &str = "limedl-native-startup";

    pub fn is_enabled() -> bool {
        if crate::update::has_package_identity() {
            return startup_task_enabled().unwrap_or(false);
        }
        winreg_is_enabled()
    }

    pub fn enable() -> anyhow::Result<()> {
        if crate::update::has_package_identity() {
            return startup_task_enable();
        }
        winreg_enable()
    }

    pub fn disable() -> anyhow::Result<()> {
        if crate::update::has_package_identity() {
            return startup_task_disable();
        }
        winreg_disable()
    }

    // ── Registry (portable / NSIS per-user installs) ──

    fn winreg_is_enabled() -> bool {
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(VALUE_NAME))
            .is_ok()
    }

    fn winreg_enable() -> anyhow::Result<()> {
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

    fn winreg_disable() -> anyhow::Result<()> {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_WRITE)
            .context("open Run key")?;
        match key.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    // ── StartupTask (MSIX / Store installs) ──
    // The StartupTask WinRT APIs must run where a package identity exists;
    // callers only reach here when `has_package_identity()` returned true.

    fn get_startup_task() -> anyhow::Result<windows::ApplicationModel::StartupTask> {
        use windows::ApplicationModel::StartupTask;
        let tasks = StartupTask::GetForCurrentPackageAsync()?.get()?;
        let count = tasks.Size()?;
        for i in 0..count {
            let task = tasks.GetAt(i)?;
            if task.TaskId()?.to_string_lossy() == STARTUP_TASK_ID {
                return Ok(task);
            }
        }
        anyhow::bail!("startup task '{STARTUP_TASK_ID}' not found in package manifest")
    }

    fn startup_task_enabled() -> anyhow::Result<bool> {
        use windows::ApplicationModel::StartupTaskState;
        let task = get_startup_task()?;
        Ok(matches!(
            task.State()?,
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy
        ))
    }

    fn startup_task_enable() -> anyhow::Result<()> {
        use windows::ApplicationModel::StartupTaskState;
        let task = get_startup_task()?;
        match task.State()? {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
            StartupTaskState::DisabledByUser => anyhow::bail!(
                "startup was disabled by the user in Task Manager; re-enable it from system settings"
            ),
            _ => {
                let new_state = task.RequestEnableAsync()?.get()?;
                if matches!(
                    new_state,
                    StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy
                ) {
                    Ok(())
                } else {
                    anyhow::bail!("startup task enable request was not granted")
                }
            }
        }
    }

    fn startup_task_disable() -> anyhow::Result<()> {
        let task = get_startup_task()?;
        task.Disable()?;
        Ok(())
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
