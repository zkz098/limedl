import { invoke } from "#invoke";

/** App identity/version info returned by `app_get_info` (dual-mode: Tauri IPC + NAS WebSocket). */
export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  arch: string;
}

/** Result of `check_update_full` (null means up to date). */
export interface CheckUpdateFullResult {
  version: string;
  body?: string;
  date?: string;
  downloadUrl: string;
  signature: string;
  currentVersion: string;
}

export function getAppInfo() {
  return invoke<AppInfo>("app_get_info");
}

/** Update tray language (dual-mode: Tauri IPC + NAS WebSocket). */
export function updateTrayLanguage(language: string) {
  return invoke<void>("update_tray_language", { language });
}

/** Check for app updates. */
export function checkUpdateFull() {
  return invoke<CheckUpdateFullResult | null>("check_update_full");
}

/** Download and install the pending update. */
export function installUpdate() {
  return invoke<void>("download_and_install_update");
}
