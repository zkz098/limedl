import { invoke } from "#invoke";
import { commandName } from "../ws/command-name";

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
  return invoke<AppSettings>(commandName("settings_get"));
}

export function saveAppSettings(settings: AppSettings) {
  return invoke<AppSettings>(commandName("settings_save"), { settings });
}

export function fetchTrackerList(trackerListUrl: string) {
  return invoke<string>(commandName("settings_fetch_tracker_list"), { trackerListUrl });
}

export function toggleGameMode(enabled: boolean) {
  return invoke<boolean>(commandName("toggle_game_mode"), { enabled });
}

export function getIoStatus() {
  return invoke<IoStatus>(commandName("get_io_status"));
}

export function detectDiskType(dir: string) {
  return invoke<"ssd" | "hdd">(commandName("detect_disk_type"), { dir });
}

export function detectAllDiskTypes() {
  return invoke<Record<string, "ssd" | "hdd">>(commandName("detect_all_disk_types"));
}

export function toggleOverclockMode(enabled: boolean) {
  return invoke<boolean>(commandName("toggle_overclock_mode"), { enabled });
}

export function getOverclockMode() {
  return invoke<boolean>(commandName("get_overclock_mode"));
}

export function factoryReset() {
  return invoke<void>(commandName("factory_reset"));
}

export function openLogDir() {
  return invoke<void>(commandName("logging_open_dir"));
}
