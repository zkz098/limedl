import { ref, computed, watch, type Ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { getAppSettings, saveAppSettings } from "../lib/tauri/settings-api";
import type { AppSettings } from "../types/settings";

interface UseSetupWizardLifecycleOptions {
  appSettings: Ref<AppSettings | null>;
  applyAppearanceSettings: (settings: AppSettings) => void;
  applyAppSettingsDefaults: (settings: AppSettings) => void;
  setNotificationsEnabled: (enabled: boolean) => void;
}

export function useSetupWizardLifecycle(options: UseSetupWizardLifecycleOptions) {
  const {
    appSettings,
    applyAppearanceSettings,
    applyAppSettingsDefaults,
    setNotificationsEnabled,
  } = options;

  const showSetupWizard = ref<boolean | null>(null);
  const setupInitialSettings = ref<AppSettings | null>(null);
  const appVersion = ref("");

  const setupStartStep = computed(() => {
    const lastStep = setupInitialSettings.value?.lastSetupStep;
    if (lastStep != null && !setupInitialSettings.value?.setupCompleted) {
      return lastStep;
    }
    return 0;
  });

  // Check localStorage cache first for instant decision
  const cachedSetupDone = localStorage.getItem("limedl.setupCompleted");
  if (cachedSetupDone === "true") {
    showSetupWizard.value = false;
  }

  function checkSetupState() {
    if (appSettings.value) {
      if (appSettings.value.setupCompleted) {
        localStorage.setItem("limedl.setupCompleted", "true");
        showSetupWizard.value = false;
      } else if (showSetupWizard.value === null) {
        // Deep copy into a plain object: appSettings is a reactive proxy from
        // the pinia store, and passing it into the wizard would make
        // structuredClone throw DataCloneError (cloning proxies is forbidden).
        // AppSettings is plain JSON data, so a JSON round-trip is safe.
        setupInitialSettings.value = JSON.parse(JSON.stringify(appSettings.value));
        showSetupWizard.value = true;
      }
    }
  }

  watch(appSettings, checkSetupState);

  async function handleSetupCompleted(settings: AppSettings) {
    // Cache in localStorage for fast boot
    localStorage.setItem("limedl.setupCompleted", "true");
    // Update the global appSettings
    appSettings.value = settings;
    // Apply appearance settings (theme, color mode) to document
    applyAppearanceSettings(settings);
    // Apply download defaults (auto-fill composer)
    applyAppSettingsDefaults(settings);
    // Notifications
    setNotificationsEnabled(settings.notifications?.enabled ?? true);
    // Hide wizard, show main app
    showSetupWizard.value = false;
  }

  async function handleSetupClosed() {
    // User closed wizard without completing (Escape key).
    // Reload settings from disk since wizard may have written partial changes.
    try {
      const updated = await getAppSettings();
      appSettings.value = updated;
      applyAppearanceSettings(updated);
    } catch {
      // If reload fails, keep the stale value — better than crashing
    }
    showSetupWizard.value = false;
  }

  async function handleRestartSetup() {
    const currentSettings = appSettings.value;
    if (!currentSettings) return;

    // Restart uses the last-persisted settings as the wizard's starting point.
    // Unsaved draft changes in the settings panel are intentionally discarded —
    // the user is explicitly choosing to re-run the setup wizard.
    localStorage.removeItem("limedl.setupCompleted");
    currentSettings.setupCompleted = false;
    currentSettings.lastSetupStep = null;

    try {
      await saveAppSettings(currentSettings);
    } catch (e) {
      console.error("Failed to reset setup state:", e);
    }

    setupInitialSettings.value = { ...currentSettings };
    showSetupWizard.value = true;
  }

  /** Call from the host component's onMounted */
  function mountSetupWizard() {
    checkSetupState();

    // Fetch real app version from Tauri metadata
    getVersion()
      .then((v) => {
        appVersion.value = v;
      })
      .catch(() => {});

    // Safety: if settings never load (backend crash, IPC failure),
    // bail out to the main app after 5 seconds instead of showing an infinite spinner
    setTimeout(() => {
      if (showSetupWizard.value === null) {
        console.warn("Settings never loaded, showing main app as fallback");
        showSetupWizard.value = false;
      }
    }, 5000);
  }

  return {
    showSetupWizard,
    setupInitialSettings,
    appVersion,
    setupStartStep,
    handleSetupCompleted,
    handleSetupClosed,
    handleRestartSetup,
    mountSetupWizard,
  };
}
