import { nextTick, onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";
import { debounce } from "es-toolkit";
import { getAppSettings, saveAppSettings } from "../lib/tauri/settings-api";
import { VALID_COLUMN_KEY_SET, DEFAULT_VISIBLE_COLUMNS } from "../lib/column-defs";
import type { AppSettings, ColorMode, SortDirection, SortKey } from "../types/settings";

export interface UseAppSettingsParams {
  sortKey: Ref<SortKey>;
  sortDirection: Ref<SortDirection>;
  compactView: Ref<boolean>;
  visibleColumns: Ref<string[]>;
  applyAppSettingsDefaults: (settings: AppSettings) => void;
  setNotificationsEnabled: (enabled: boolean) => void;
}

function resolveColorMode(mode: ColorMode): "light" | "dark" {
  if (mode !== "system") {
    return mode;
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function useAppSettings(params: UseAppSettingsParams) {
  const {
    sortKey,
    sortDirection,
    compactView,
    visibleColumns,
    applyAppSettingsDefaults,
    setNotificationsEnabled,
  } = params;

  const appSettings = ref<AppSettings | null>(null);
  const isSyncingFromSettings = ref(false);

  let colorSchemeQuery: MediaQueryList | null = null;

  function applyColorMode(mode: ColorMode) {
    document.documentElement.dataset.colorModePreference = mode;
    document.documentElement.dataset.colorMode = resolveColorMode(mode);
  }

  function applyAppearanceSettings(settings: AppSettings) {
    document.documentElement.dataset.theme = settings.appearance?.themeColor ?? "lime";
    document.documentElement.dataset.surface = settings.appearance?.backgroundOpacity ?? "default";
    applyColorMode(settings.appearance?.colorMode ?? "system");
  }

  function handleSystemColorSchemeChange() {
    applyColorMode(appSettings.value?.appearance?.colorMode ?? "system");
  }

  async function loadSettings() {
    try {
      appSettings.value = await getAppSettings();
      applyAppearanceSettings(appSettings.value);
      applyAppSettingsDefaults(appSettings.value);
    } catch (error) {
      console.error("Failed to load app settings", error);
    }
  }

  // Watcher: sync appSettings → notifications
  watch(
    appSettings,
    (settings) => {
      setNotificationsEnabled(settings?.notifications?.enabled ?? false);
    },
    { immediate: true },
  );

  // Watcher: sync appSettings → sortKey, sortDirection, compactView, visibleColumns
  watch(
    appSettings,
    (settings) => {
      if (!settings) {
        return;
      }

      isSyncingFromSettings.value = true;
      sortKey.value = settings.appearance?.sortKey ?? "added_at";
      sortDirection.value = settings.appearance?.sortDirection ?? "desc";
      compactView.value = settings.appearance?.compactView ?? false;
      const loaded = (settings.appearance?.visibleColumns ?? [...DEFAULT_VISIBLE_COLUMNS]).filter(
        (k) => VALID_COLUMN_KEY_SET.has(k),
      );
      if (!loaded.includes("file")) {
        loaded.unshift("file");
      }
      visibleColumns.value = loaded;
      void nextTick(() => {
        isSyncingFromSettings.value = false;
      });
    },
    { immediate: true },
  );

  // Debounced save for view preferences
  const debouncedSaveAppearance = debounce(async () => {
    if (!appSettings.value) return;
    try {
      await saveAppSettings(appSettings.value);
    } catch (error) {
      console.error("Failed to save view settings", error);
    }
  }, 300);

  // Watcher: sync sortKey, sortDirection, compactView, visibleColumns → appSettings (debounced save)
  watch([sortKey, sortDirection, compactView, visibleColumns], () => {
    if (isSyncingFromSettings.value || !appSettings.value) {
      return;
    }

    appSettings.value.appearance.sortKey = sortKey.value;
    appSettings.value.appearance.sortDirection = sortDirection.value;
    appSettings.value.appearance.compactView = compactView.value;
    appSettings.value.appearance.visibleColumns = [...visibleColumns.value];

    debouncedSaveAppearance();
  });

  onMounted(() => {
    colorSchemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
    colorSchemeQuery.addEventListener("change", handleSystemColorSchemeChange);
    applyColorMode("system");
    void loadSettings();
  });

  onBeforeUnmount(() => {
    debouncedSaveAppearance.cancel();
    colorSchemeQuery?.removeEventListener("change", handleSystemColorSchemeChange);
  });

  return {
    appSettings,
    applyAppearanceSettings,
  };
}
