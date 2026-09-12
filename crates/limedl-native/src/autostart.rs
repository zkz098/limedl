#![allow(dead_code)]
// Native autostart integration — OS-specific implementation.
// Windows portable/NSIS: HKCU\Software\Microsoft\Windows\CurrentVersion\Run (registry)
// Windows MSIX/Store:    windows.startupTask manifest extension + StartupTask API
//                        (registry Run is virtualized inside MSIX and would be lost)
// Linux:   ~/.config/autostart/limedl-native.desktop (XDG)
// macOS:   ~/Library/LaunchAgents/com.zkz20.limedl.plist
//
// Every registration passes `--hidden` so a login start goes straight to the
// tray instead of popping the window (see `start_hidden` in main.rs).

/// Argument appended to every autostart registration.
pub const HIDDEN_ARG: &str = "--hidden";

fn current_exe_string() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Windows registry value / .desktop Exec / plist argument string for the
/// current executable, always with the hidden flag.
fn expected_command() -> Option<String> {
    let exe = current_exe_string()?;
    Some(if exe.contains(' ') {
        format!("\"{exe}\" {HIDDEN_ARG}")
    } else {
        format!("{exe} {HIDDEN_ARG}")
    })
}

/// True when `stored` (an OS registration blob) already points at the current
/// executable and carries the hidden flag. Used to re-register after the app
/// moved (portable zip extraction to a new folder, debug vs release build, …).
fn registration_is_current(stored: &str) -> bool {
    let Some(exe) = current_exe_string() else {
        return false;
    };
    let normalize = |s: &str| s.replace('\\', "/").to_lowercase();
    let stored_norm = normalize(stored);
    stored_norm.contains(&normalize(&exe)) && stored.contains(HIDDEN_ARG)
}

#[cfg(windows)]
mod windows_impl {
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
        let val = super::expected_command().context("failed to get current exe path")?;
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_WRITE)
            .context("open Run key")?
            .set_value(VALUE_NAME, &val)
            .context("set Run value")?;
        Ok(())
    }

    /// True when the HKCU Run entry already points at this executable with the
    /// hidden flag (a stale path is re-registered by `sync_from_settings`).
    pub fn registered_for_current_exe() -> bool {
        if crate::update::has_package_identity() {
            // MSIX: the StartupTask manifest owns the command line, so there is
            // nothing to reconcile here.
            return true;
        }
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(VALUE_NAME))
            .is_ok_and(|stored| super::registration_is_current(&stored))
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
        let path = autostart_file().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        let exe = super::expected_command().ok_or_else(|| anyhow::anyhow!("no exe"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=limedl-native\nExec={exe}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n"
        );
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// True when the .desktop file already points at this executable with the
    /// hidden flag.
    pub fn registered_for_current_exe() -> bool {
        autostart_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|content| super::registration_is_current(&content))
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
    pub fn registered_for_current_exe() -> bool {
        true
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use std::path::PathBuf;

    fn plist_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/LaunchAgents/com.zkz20.limedl.plist"))
    }

    pub fn is_enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }

    pub fn enable() -> anyhow::Result<()> {
        let path = plist_path().ok_or_else(|| anyhow::anyhow!("no home"))?;
        let exe = super::current_exe_string().ok_or_else(|| anyhow::anyhow!("no exe"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.zkz20.limedl</string>
    <key>ProgramArguments</key><array><string>{exe}</string><string>{}</string></array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
            super::HIDDEN_ARG
        );
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// True when the LaunchAgent already points at this executable with the
    /// hidden flag.
    pub fn registered_for_current_exe() -> bool {
        plist_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|content| super::registration_is_current(&content))
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
    pub fn registered_for_current_exe() -> bool {
        true
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

/// True when the existing OS registration already launches this executable with
/// the `--hidden` flag (used to repair stale paths, e.g. after a portable
/// install moved to another folder).
pub fn registered_for_current_exe() -> bool {
    #[cfg(windows)]
    {
        windows_impl::registered_for_current_exe()
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::registered_for_current_exe()
    }
    #[cfg(target_os = "macos")]
    {
        macos_impl::registered_for_current_exe()
    }
    #[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
    {
        true
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
///
/// Besides the on/off flag this also repairs a stale registration: if autostart
/// is enabled but the stored command does not reference the current executable
/// (or is missing `--hidden`), it is rewritten.
pub fn sync_from_settings(autostart_flag: bool) {
    let os_enabled = is_enabled();
    if autostart_flag != os_enabled {
        let res = set_enabled(autostart_flag);
        if let Err(e) = res {
            tracing::warn!("autostart sync failed (want={autostart_flag}, os={os_enabled}): {e:#}");
        } else {
            tracing::info!("autostart sync: set OS autostart to {autostart_flag}");
        }
        return;
    }

    if autostart_flag && os_enabled && !registered_for_current_exe() {
        match enable() {
            Ok(()) => tracing::info!("autostart sync: re-registered the current executable"),
            Err(e) => tracing::warn!("autostart re-registration failed: {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_command_contains_hidden_flag() {
        let Some(cmd) = expected_command() else {
            return; // exe path unavailable in this environment
        };
        assert!(cmd.ends_with(HIDDEN_ARG), "unexpected command: {cmd}");
        // Paths with spaces must be quoted so the shell splits them correctly.
        if let Some(exe) = current_exe_string()
            && exe.contains(' ')
        {
            assert!(cmd.starts_with('"'), "unquoted spaced path: {cmd}");
        }
    }

    #[test]
    fn registration_is_current_checks_path_and_flag() {
        let Some(exe) = current_exe_string() else {
            return;
        };
        let quoted = format!("\"{exe}\" {HIDDEN_ARG}");
        assert!(registration_is_current(&quoted));
        assert!(registration_is_current(&format!("{exe} {HIDDEN_ARG}")));
        // Forward slashes are tolerated (XDG/plist may store either).
        assert!(registration_is_current(&format!(
            "{} {HIDDEN_ARG}",
            exe.replace('\\', "/")
        )));
        // Wrong executable → stale registration.
        assert!(!registration_is_current(&format!(
            "C:/other/limedl-native.exe {HIDDEN_ARG}"
        )));
        // Right executable but missing the hidden flag → needs a rewrite.
        assert!(!registration_is_current(&exe));
    }
}
