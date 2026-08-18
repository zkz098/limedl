import { invoke } from "#invoke";
import { commandName } from "../ws/command-name";

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
  return invoke<AppInfo>(commandName("app_get_info"));
}

/** Update tray language (dual-mode: Tauri IPC + NAS WebSocket). */
export function updateTrayLanguage(language: string) {
  return invoke<void>(commandName("update_tray_language"), { language });
}

/** Check for app updates. */
export function checkUpdateFull() {
  return invoke<CheckUpdateFullResult | null>(commandName("check_update_full"));
}

/** Download and install the pending update. */
export function installUpdate() {
  return invoke<void>(commandName("download_and_install_update"));
}
