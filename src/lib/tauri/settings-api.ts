import { invoke } from "@tauri-apps/api/core";

import type { ProxySettings } from "../../types/settings";

export function getProxySettings() {
  return invoke<ProxySettings>("settings_proxy_get");
}

export function saveProxySettings(settings: ProxySettings) {
  return invoke<ProxySettings>("settings_proxy_save", { settings });
}
