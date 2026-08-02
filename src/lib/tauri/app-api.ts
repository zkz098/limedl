import { invoke } from "#invoke";

/** App identity/version info returned by `app_get_info` (dual-mode: Tauri IPC + NAS WebSocket). */
export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  arch: string;
}

export function getAppInfo() {
  return invoke<AppInfo>("app_get_info");
}
