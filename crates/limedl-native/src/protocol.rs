//! System protocol association for limedl (`magnet:?` and `limedl://`).
//!
//! Registers protocol schemes in `HKEY_CURRENT_USER\Software\Classes` so that
//! clicking a magnet link in a web browser or external application opens limedl.
//!
//! Because this writes to `HKEY_CURRENT_USER`, no administrative privileges
//! or UAC elevation are required.
//!
//! Registration only makes sense on Windows, where the stubs below are replaced by
//! the registry implementation — on other platforms the entry points are dead code
//! by design.
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
const MAGNET_KEY: &str = r"Software\Classes\magnet";
#[cfg(windows)]
const LIMEDL_KEY: &str = r"Software\Classes\limedl";

/// Check if the `magnet:` protocol is registered to the current limedl executable.
#[allow(dead_code)]
pub fn is_magnet_registered() -> bool {
    #[cfg(windows)]
    {
        if let Ok(key) = windows_registry::CURRENT_USER
            .open(format!(r"{MAGNET_KEY}\shell\open\command"))
            && let Ok(cmd) = key.get_string("")
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
        let cmd = format!("\"{exe_path}\" \"%1\"");

        let hkcu = windows_registry::CURRENT_USER;

        // 1. magnet:? protocol
        let magnet_key = hkcu.create(MAGNET_KEY)?;
        magnet_key.set_string("", "URL:BitTorrent Magnet Link")?;
        magnet_key.set_string("URL Protocol", "")?;
        let cmd_key = hkcu.create(format!(r"{MAGNET_KEY}\shell\open\command"))?;
        cmd_key.set_string("", &cmd)?;

        // 2. limedl:// deep link
        let limedl_key = hkcu.create(LIMEDL_KEY)?;
        limedl_key.set_string("", "URL:limedl Protocol")?;
        limedl_key.set_string("URL Protocol", "")?;
        let limedl_cmd_key = hkcu.create(format!(r"{LIMEDL_KEY}\shell\open\command"))?;
        limedl_cmd_key.set_string("", &cmd)?;

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
        let hkcu = windows_registry::CURRENT_USER;
        let _ = hkcu.remove_tree(MAGNET_KEY);
        let _ = hkcu.remove_tree(LIMEDL_KEY);
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
