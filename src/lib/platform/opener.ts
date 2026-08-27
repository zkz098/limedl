import { openUrl as tauriOpenUrl } from "@tauri-apps/plugin-opener";
import { isTauri } from "./env";

export async function openUrl(url: string): Promise<void> {
  if (isTauri() && typeof tauriOpenUrl === "function") {
    return tauriOpenUrl(url);
  }
  if (typeof window !== "undefined") {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
