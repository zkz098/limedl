import { invoke } from "@tauri-apps/api/core";

import type { AppSettings } from "../../types/settings";

export interface IoStatus {
  gameMode: boolean;
  bufferUsageBytes: number;
  bufferLimitBytes: number;
  degradationCount: number;
}

export function getAppSettings() {
  return invoke<AppSettings>("settings_get");
}

export function saveAppSettings(settings: AppSettings) {
  return invoke<AppSettings>("settings_save", { settings });
}

export function fetchTrackerList(trackerListUrl: string) {
  return invoke<string>("settings_fetch_tracker_list", { trackerListUrl });
}

export function toggleGameMode(enabled: boolean) {
  return invoke<boolean>("toggle_game_mode", { enabled });
}

export function getIoStatus() {
  return invoke<IoStatus>("get_io_status");
}
