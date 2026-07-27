import { invoke } from "#invoke";

import type { AppSettings } from "../../types/settings";

export interface IoStatus {
  gameMode: boolean;
  bufferUsageBytes: number;
  bufferLimitBytes: number;
  degradationCount: number;
  activeSlots: number;
  maxSlots: number;
  queuedCount: number;
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

export function detectDiskType(dir: string) {
  return invoke<"ssd" | "hdd">("detect_disk_type", { dir });
}

export function detectAllDiskTypes() {
  return invoke<Record<string, "ssd" | "hdd">>("detect_all_disk_types");
}

export function toggleOverclockMode(enabled: boolean) {
  return invoke<boolean>("toggle_overclock_mode", { enabled });
}

export function getOverclockMode() {
  return invoke<boolean>("get_overclock_mode");
}

export function factoryReset() {
  return invoke<void>("factory_reset");
}
