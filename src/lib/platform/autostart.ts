/**
 * Autostart registration.
 *
 * Autostart is a desktop-client concern: `limedl-native` applies
 * `settings.autostart` in Rust (Windows Run key / LaunchAgent / .desktop file).
 * The WebUI only persists the setting, so these are deliberate no-ops here.
 */

export async function isAutostartEnabled(): Promise<boolean> {
  return false;
}

export async function enableAutostart(): Promise<void> {}

export async function disableAutostart(): Promise<void> {}
