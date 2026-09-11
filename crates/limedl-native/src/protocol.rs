//! System protocol association for limedl (`magnet:?` and `limedl://`).
//!
//! Registers protocol schemes in `HKEY_CURRENT_USER\Software\Classes` so that
//! clicking a magnet link in a web browser or external application opens limedl.
//!
//! Because this writes to `HKEY_CURRENT_USER`, no administrative privileges
//! or UAC elevation are required.

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
const MAGNET_KEY: &str = "Software\\Classes\\magnet";
#[cfg(windows)]
const LIMEDL_KEY: &str = "Software\\Classes\\limedl";

/// Check if the `magnet:` protocol is registered to the current limedl executable.
#[allow(dead_code)]
pub fn is_magnet_registered() -> bool {
    #[cfg(windows)]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey_with_flags(
            format!("{MAGNET_KEY}\\shell\\open\\command"),
            KEY_READ,
        ) && let Ok(cmd) = key.get_value::<String, _>("")
            && let Ok(current_exe) = std::env::current_exe()
        {
            let exe_str = current_exe.to_string_lossy();
            return cmd.contains(&*exe_str);
        }
        false
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Register `magnet:` and `limedl:` protocol schemes to the current executable.
pub fn register_protocols() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let current_exe = std::env::current_exe()?;
        let exe_path = current_exe.to_string_lossy();
        let cmd = format!("\"{}\" \"%1\"", exe_path);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        // 1. magnet:? protocol
        let (magnet_key, _) = hkcu.create_subkey(MAGNET_KEY)?;
        magnet_key.set_value("", &"URL:BitTorrent Magnet Link")?;
        magnet_key.set_value("URL Protocol", &"")?;
        let (cmd_key, _) = hkcu.create_subkey(format!("{MAGNET_KEY}\\shell\\open\\command"))?;
        cmd_key.set_value("", &cmd)?;

        // 2. limedl:// deep link
        let (limedl_key, _) = hkcu.create_subkey(LIMEDL_KEY)?;
        limedl_key.set_value("", &"URL:limedl Protocol")?;
        limedl_key.set_value("URL Protocol", &"")?;
        let (limedl_cmd_key, _) =
            hkcu.create_subkey(format!("{LIMEDL_KEY}\\shell\\open\\command"))?;
        limedl_cmd_key.set_value("", &cmd)?;

        tracing::info!("系统协议关联已成功注册 (magnet:, limedl:)");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

/// Remove `magnet:` and `limedl:` protocol schemes from user registry.
#[allow(dead_code)]
pub fn unregister_protocols() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(MAGNET_KEY);
        let _ = hkcu.delete_subkey_all(LIMEDL_KEY);
        tracing::info!("系统协议关联已移除");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_api_compiles() {
        // Just verify functions can be invoked without panic
        let _ = is_magnet_registered();
    }
}
