import { relaunch as tauriRelaunch, exit as tauriExit } from "@tauri-apps/plugin-process";
import { version as tauriOsVersion } from "@tauri-apps/plugin-os";
import { getTauriVersion as tauriGetTauriVersion } from "@tauri-apps/api/app";
import { isTauri } from "./env";

export async function relaunchApp(): Promise<void> {
  if (isTauri() && typeof tauriRelaunch === "function") {
    return tauriRelaunch();
  }
  if (typeof window !== "undefined") {
    window.location.reload();
  }
}

export async function exitApp(code = 0): Promise<void> {
  if (isTauri() && typeof tauriExit === "function") {
    return tauriExit(code);
  }
  if (typeof window !== "undefined") {
    window.close();
  }
}

export async function getPlatformOsVersion(): Promise<string> {
  if (isTauri() && typeof tauriOsVersion === "function") {
    return tauriOsVersion();
  }
  return typeof navigator !== "undefined" ? navigator.userAgent : "web";
}

export async function getPlatformTauriVersion(): Promise<string> {
  if (isTauri() && typeof tauriGetTauriVersion === "function") {
    return tauriGetTauriVersion();
  }
  return "N/A (Web/NAS)";
}
