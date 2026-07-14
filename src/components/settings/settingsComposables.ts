// Re-export barrel — split by concern into focused modules:
//   useSettingsForm.ts        — settings form state, sync, dirty tracking composable
//   useSettingsNotification.ts — notification utility composable
//   useSettingsSummaries.ts   — settings summary/statistics composable
//   settingsUtils.ts          — standalone serialize/copy/snapshot utilities

export { useSettingsForm } from "./useSettingsForm";
export { useSettingsNotification } from "./useSettingsNotification";
export {
  useSettingsSummaries,
  DEFAULT_TRACKER_LIST_URL,
  DEFAULT_HTTP_USER_AGENT,
  type SettingsOptionArrays,
} from "./useSettingsSummaries";
export { serializeSettings, settingsDraftSnapshot } from "./settingsUtils";
