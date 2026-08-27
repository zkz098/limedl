import {
  enable as tauriEnable,
  disable as tauriDisable,
  isEnabled as tauriIsEnabled,
} from "@tauri-apps/plugin-autostart";
import { isTauri } from "./env";

export async function isAutostartEnabled(): Promise<boolean> {
  if (isTauri() && typeof tauriIsEnabled === "function") {
    return tauriIsEnabled();
  }
  return false;
}

export async function enableAutostart(): Promise<void> {
  if (isTauri() && typeof tauriEnable === "function") {
    return tauriEnable();
  }
}

export async function disableAutostart(): Promise<void> {
  if (isTauri() && typeof tauriDisable === "function") {
    return tauriDisable();
  }
}
