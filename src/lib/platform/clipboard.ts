import {
  readText as tauriReadText,
  writeText as tauriWriteText,
} from "@tauri-apps/plugin-clipboard-manager";
import { isTauri } from "./env";

export async function readClipboardText(): Promise<string> {
  if (isTauri() && typeof tauriReadText === "function") {
    return tauriReadText();
  }
  if (typeof navigator !== "undefined" && navigator.clipboard?.readText) {
    try {
      return await navigator.clipboard.readText();
    } catch {
      return "";
    }
  }
  return "";
}

export async function writeClipboardText(text: string): Promise<void> {
  if (isTauri() && typeof tauriWriteText === "function") {
    return tauriWriteText(text);
  }
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
  }
}
