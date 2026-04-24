import { invoke } from "@tauri-apps/api/core";

import type { AppSettings } from "../../types/settings";

export function getAppSettings() {
  return invoke<AppSettings>("settings_get");
}

export function saveAppSettings(settings: AppSettings) {
  return invoke<AppSettings>("settings_save", { settings });
}

export function fetchTrackerList(trackerListUrl: string) {
  return invoke<string>("settings_fetch_tracker_list", { trackerListUrl });
}
