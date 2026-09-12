import { invoke } from "#invoke";
import { commandName } from "../ws/command-name";

/** App identity/version info returned by `app_get_info`. */
export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  arch: string;
}

export function getAppInfo() {
  return invoke<AppInfo>(commandName("app_get_info"));
}

/** Update tray language. */
export function updateTrayLanguage(language: string) {
  return invoke<void>(commandName("update_tray_language"), { language });
}
