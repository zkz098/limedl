/**
 * Runtime environment detection for Tauri desktop vs NAS WebUI.
 */
export function isTauri(): boolean {
  if (typeof window === "undefined") return false;
  if ("__TAURI_INTERNALS__" in window || "__TAURI__" in window) return true;
  // Vitest test runner / test mode
  if (import.meta.env.MODE === "test") {
    return true;
  }
  return false;
}
