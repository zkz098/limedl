import type { AppSettings } from "../../types/settings";

export function serializeSettings(settings: AppSettings) {
  return JSON.stringify(settings);
}

export function settingsDraftSnapshot(settings: AppSettings) {
  return serializeSettings(settings);
}
