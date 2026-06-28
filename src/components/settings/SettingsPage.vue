<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import { useI18n } from "../../i18n";
import { useNotification } from "../../composables/useNotification";
import { pickDirectory } from "../../lib/tauri/dialog-api";
import { fetchTrackerList, saveAppSettings } from "../../lib/tauri/settings-api";
import type { ChecksumMode } from "../../types/download";
import type { SupportedLanguage } from "../../i18n/resources";
import type {
  AdaptiveProfile,
  AppSettings,
  BackgroundOpacityPreset,
  ColorMode,
  DeviceLearningMode,
  LogLevel,
  NetworkLearningSettings,
  ProxyMode,
  SchedulerMode,
} from "../../types/settings";
import NotificationToast from "../ui/NotificationToast.vue";
import UiButton from "../ui/UiButton.vue";

import SettingsAppearancePanel from "./SettingsAppearancePanel.vue";
import SettingsAria2RpcPanel from "./SettingsAria2RpcPanel.vue";
import SettingsBtPanel from "./SettingsBtPanel.vue";
import SettingsDownloadDefaultsPanel from "./SettingsDownloadDefaultsPanel.vue";
import SettingsLoggingPanel from "./SettingsLoggingPanel.vue";
import SettingsProxyPanel from "./SettingsProxyPanel.vue";
import SettingsSchedulerPanel from "./SettingsSchedulerPanel.vue";

import {
  copySingleNetworkScene,
  DEFAULT_HTTP_USER_AGENT,
  DEFAULT_TRACKER_LIST_URL,
  serializeSettings,
  settingsDraftSnapshot,
  useSettingsSummaries,
  type SettingsOptionArrays,
} from "./settingsComposables";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
  dirtyChange: [isDirty: boolean];
}>();

const { language, languageOptions, setLanguage, t } = useI18n();
const { notifications, notifySuccess, notifyError, notifyInfo, dismiss } = useNotification();

// ── Option arrays ────────────────────────────────────────────────

const proxyModeOptions = computed<Array<{ label: string; value: ProxyMode }>>(() => [
  { label: t("tokens.disabled"), value: "disabled" },
  { label: t("tokens.system"), value: "system" },
  { label: t("tokens.manual"), value: "manual" },
]);

const schedulerModeOptions = computed<Array<{ label: string; value: SchedulerMode }>>(() => [
  { label: t("tokens.automatic"), value: "automatic" },
  { label: t("tokens.traditional"), value: "traditional" },
]);

const adaptiveProfileOptions = computed<Array<{ label: string; value: AdaptiveProfile }>>(() => [
  { label: t("tokens.conservative"), value: "conservative" },
  { label: t("tokens.balanced"), value: "balanced" },
  { label: t("tokens.aggressive"), value: "aggressive" },
]);

const checksumOptions = computed<Array<{ label: string; value: ChecksumMode }>>(() => [
  { label: t("tokens.blake3"), value: "blake3" },
  { label: t("tokens.sha256"), value: "sha256" },
  { label: t("tokens.xxh3_128"), value: "xxh3_128" },
  { label: t("tokens.none"), value: "none" },
]);

const deviceModeOptions = computed<Array<{ label: string; value: DeviceLearningMode }>>(() => [
  { label: t("tokens.fixed"), value: "fixed" },
  { label: t("tokens.mobile"), value: "mobile" },
  { label: t("tokens.semi_mobile"), value: "semi_mobile" },
]);

const logLevelOptions = computed<Array<{ label: string; value: LogLevel }>>(() => [
  { label: t("tokens.trace"), value: "trace" },
  { label: t("tokens.debug"), value: "debug" },
  { label: t("tokens.info"), value: "info" },
  { label: t("tokens.warn"), value: "warn" },
  { label: t("tokens.error"), value: "error" },
]);

const backgroundOpacityOptions = computed<Array<{ label: string; value: BackgroundOpacityPreset }>>(
  () => [
    { label: t("settings.backgroundOpacityNames.default"), value: "default" },
    { label: t("settings.backgroundOpacityNames.acrylic"), value: "acrylic" },
    { label: t("settings.backgroundOpacityNames.frosted"), value: "frosted" },
  ],
);

const colorModeOptions = computed<Array<{ label: string; value: ColorMode }>>(() => [
  { label: t("settings.colorModeNames.system"), value: "system" },
  { label: t("settings.colorModeNames.light"), value: "light" },
  { label: t("settings.colorModeNames.dark"), value: "dark" },
]);

// ── Reactive form ─────────────────────────────────────────────────

const form = reactive<AppSettings>({
  globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "default",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: true,
      showHeatmap: true,
    },
  proxy: {
    mode: "disabled",
    manualUrl: "",
  },
  scheduler: {
    mode: "automatic",
    traditional: {
      maxParallelTasks: 3,
    },
    automatic: {
      maxParallelThreads: 16,
      maxThreadsPerTask: 8,
      minThreadsPerTask: 0,
      adaptiveProfile: "balanced",
    },
  },
  download: {
    defaultDownloadDir: "",
    defaultMaxRetries: 5,
    defaultChecksum: "blake3",
    defaultUserAgent: DEFAULT_HTTP_USER_AGENT,
    enableMetalink: false,
    enableSftp: false,
  },
  bt: {
    dhtEnabled: true,
    trackerList: "",
    trackerListUrl: DEFAULT_TRACKER_LIST_URL,
    pauseUploadWhenLimitReached: false,
    uploadLimitBytes: 0,
    uploadRatioLimit: 0,
    defaultDownloadSpeedLimit: 0,
    defaultUploadSpeedLimit: 0,
  },
  networkLearning: {
    deviceMode: "fixed",
    currentSceneId: "default",
    scenes: [
      {
        id: "default",
        name: t("settings.defaultScene"),
        learningEnabled: true,
        learnedMetrics: null,
        updatedAtMs: 0,
      },
    ],
  },
  logging: {
    enabled: true,
    level: "info",
    filePath: "",
  },
  aria2Rpc: {
    enabled: true,
    port: 6800,
    secret: null,
  },
  cdnAcceleration: {
    enabled: false,
    activeIp: null,
    activeSpeedMbps: null,
    lastTestAtMs: null,
    lastError: null,
  },
});

// ── State──────────────────────────────────────────────────────────

const isSaving = ref(false);
const isPickingDirectory = ref(false);
const isFetchingTrackerList = ref(false);
const savedSettingsSnapshot = ref("");

// ── Summaries (from composable)────────────────────────────────────

const optionArrays: SettingsOptionArrays = {
  adaptiveProfileOptions,
  deviceModeOptions,
  checksumOptions,
  logLevelOptions,
};

const {
  pageSummary,
  proxySummary,
  loggingSummary,
  downloadSummary,
  btUploadLimitMiB,
  setBtUploadLimitMiB,
  globalSpeedLimitMiBps,
  setGlobalSpeedLimitMiBps,
  trackerListEntries,
  btSummary,
} = useSettingsSummaries(form, t, optionArrays);

const settingsDraftSnapshotComputed = computed(() => settingsDraftSnapshot(buildSettingsPayload()));

// ── Settings sync watcher ─────────────────────────────────────────

watch(
  () => props.settings,
  (nextSettings) => {
    if (!nextSettings) {
      return;
    }

    form.globalSpeedLimitBps = nextSettings.globalSpeedLimitBps ?? 0;
    form.appearance.themeColor = nextSettings.appearance?.themeColor ?? "default";
    form.appearance.backgroundOpacity = nextSettings.appearance?.backgroundOpacity ?? "default";
    form.appearance.colorMode = nextSettings.appearance?.colorMode ?? "system";
    form.appearance.showDetailInfo = nextSettings.appearance?.showDetailInfo ?? true;
    form.appearance.showHeatmap = nextSettings.appearance?.showHeatmap ?? true;
    form.proxy.mode = nextSettings.proxy.mode;
    form.proxy.manualUrl = nextSettings.proxy.manualUrl;
    form.scheduler.mode = nextSettings.scheduler.mode;
    form.scheduler.traditional.maxParallelTasks =
      nextSettings.scheduler.traditional.maxParallelTasks;
    form.scheduler.automatic.maxParallelThreads =
      nextSettings.scheduler.automatic.maxParallelThreads;
    form.scheduler.automatic.maxThreadsPerTask = nextSettings.scheduler.automatic.maxThreadsPerTask;
    form.scheduler.automatic.minThreadsPerTask =
      nextSettings.scheduler.automatic.minThreadsPerTask ?? 0;
    form.scheduler.automatic.adaptiveProfile = nextSettings.scheduler.automatic.adaptiveProfile;
    form.download.defaultDownloadDir = nextSettings.download.defaultDownloadDir;
    form.download.defaultMaxRetries = nextSettings.download.defaultMaxRetries;
    form.download.defaultChecksum = nextSettings.download.defaultChecksum;
    form.download.defaultUserAgent =
      nextSettings.download.defaultUserAgent || DEFAULT_HTTP_USER_AGENT;
    form.download.enableMetalink = nextSettings.download.enableMetalink ?? false;
    form.download.enableSftp = nextSettings.download.enableSftp ?? false;
    form.bt.dhtEnabled = nextSettings.bt.dhtEnabled;
    form.bt.trackerList = nextSettings.bt.trackerList;
    form.bt.trackerListUrl = nextSettings.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL;
    form.bt.pauseUploadWhenLimitReached = nextSettings.bt.pauseUploadWhenLimitReached;
    form.bt.uploadLimitBytes = nextSettings.bt.uploadLimitBytes;
    form.bt.uploadRatioLimit = nextSettings.bt.uploadRatioLimit;
    form.bt.defaultDownloadSpeedLimit = nextSettings.bt.defaultDownloadSpeedLimit ?? 0;
    form.bt.defaultUploadSpeedLimit = nextSettings.bt.defaultUploadSpeedLimit ?? 0;
    form.networkLearning.deviceMode = nextSettings.networkLearning.deviceMode;
    form.networkLearning.currentSceneId = "default";
    form.networkLearning.scenes = [copyNetworkScene(nextSettings.networkLearning)];
    form.logging.enabled = nextSettings.logging?.enabled ?? true;
    form.logging.level = nextSettings.logging?.level ?? "info";
    form.logging.filePath = nextSettings.logging?.filePath ?? "";
    form.aria2Rpc.enabled = nextSettings.aria2Rpc?.enabled ?? true;
    form.aria2Rpc.port = nextSettings.aria2Rpc?.port ?? 6800;
    form.aria2Rpc.secret = nextSettings.aria2Rpc?.secret ?? null;
    form.cdnAcceleration = { ...nextSettings.cdnAcceleration };
    savedSettingsSnapshot.value = serializeSettings(buildSettingsPayload());
    emit("dirtyChange", false);
  },
  { immediate: true },
);

// ── Dirty tracking ────────────────────────────────────────────────

watch(
  settingsDraftSnapshotComputed,
  (snapshot) => {
    if (!savedSettingsSnapshot.value) {
      return;
    }

    emit("dirtyChange", snapshot !== savedSettingsSnapshot.value);
  },
  { immediate: true },
);

// ── Constraint: maxThreadsPerTask ≤ maxParallelThreads ────────────

watch(
  () => form.scheduler.automatic.maxParallelThreads,
  (value) => {
    if (form.scheduler.automatic.maxThreadsPerTask > value) {
      form.scheduler.automatic.maxThreadsPerTask = value;
    }
  },
);

// ── Helpers ───────────────────────────────────────────────────────

function copyNetworkScene(settings: NetworkLearningSettings) {
  return copySingleNetworkScene(settings, t);
}

function changeLanguage(nextLanguage: SupportedLanguage) {
  void setLanguage(nextLanguage);
}

function buildSettingsPayload(): AppSettings {
  return {
    globalSpeedLimitBps: form.globalSpeedLimitBps,
      appearance: {
        themeColor: form.appearance.themeColor,
        backgroundOpacity: form.appearance.backgroundOpacity,
        colorMode: form.appearance.colorMode,
        showDetailInfo: form.appearance.showDetailInfo,
        showHeatmap: form.appearance.showHeatmap,
      },
    proxy: {
      mode: form.proxy.mode,
      manualUrl: form.proxy.manualUrl,
    },
    scheduler: {
      mode: form.scheduler.mode,
      traditional: {
        maxParallelTasks: form.scheduler.traditional.maxParallelTasks,
      },
      automatic: {
        maxParallelThreads: form.scheduler.automatic.maxParallelThreads,
        maxThreadsPerTask: form.scheduler.automatic.maxThreadsPerTask,
        minThreadsPerTask: form.scheduler.automatic.minThreadsPerTask,
        adaptiveProfile: form.scheduler.automatic.adaptiveProfile,
      },
    },
    download: {
      defaultDownloadDir: form.download.defaultDownloadDir,
      defaultMaxRetries: form.download.defaultMaxRetries,
      defaultChecksum: form.download.defaultChecksum,
      defaultUserAgent: form.download.defaultUserAgent,
      enableMetalink: form.download.enableMetalink,
      enableSftp: form.download.enableSftp,
    },
    bt: {
      dhtEnabled: form.bt.dhtEnabled,
      trackerList: form.bt.trackerList,
      trackerListUrl: form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL,
      pauseUploadWhenLimitReached: form.bt.pauseUploadWhenLimitReached,
      uploadLimitBytes: form.bt.uploadLimitBytes,
      uploadRatioLimit: form.bt.uploadRatioLimit,
      // TODO: Add defaultDownloadSpeedLimit and defaultUploadSpeedLimit to save payload
      // once backend BtSettings in src-tauri/src/download/types.rs supports them.
    },
    networkLearning: {
      deviceMode: form.networkLearning.deviceMode,
      currentSceneId: "default",
      scenes: [copyNetworkScene(form.networkLearning)],
    },
    logging: {
      enabled: form.logging.enabled,
      level: form.logging.level,
      filePath: form.logging.filePath.trim(),
    },
    aria2Rpc: {
      enabled: form.aria2Rpc.enabled,
      port: form.aria2Rpc.port,
      secret: form.aria2Rpc.secret?.trim() || null,
    },
    cdnAcceleration: {
      ...form.cdnAcceleration,
    },
  };
}

// ── Actions ───────────────────────────────────────────────────────

async function pickDefaultDownloadDirectory() {
  if (isPickingDirectory.value) {
    return;
  }

  isPickingDirectory.value = true;

  try {
    const selectedPath = await pickDirectory();
    if (selectedPath) {
      form.download.defaultDownloadDir = selectedPath;
    }
  } catch (error) {
    notifyError(
      error instanceof Error ? error.message : t("settings.notifications.chooseDirectoryFailed"),
    );
  } finally {
    isPickingDirectory.value = false;
  }
}

async function updateTrackerListFromUrl() {
  if (isFetchingTrackerList.value) {
    return;
  }

  isFetchingTrackerList.value = true;

  try {
    form.bt.trackerList = await fetchTrackerList(
      form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL,
    );
    notifySuccess(
      t("settings.notifications.trackerListUpdated", {
        count: trackerListEntries.value.length,
      }),
    );
  } catch (error) {
    notifyError(
      error instanceof Error ? error.message : t("settings.notifications.trackerListUpdateFailed"),
    );
  } finally {
    isFetchingTrackerList.value = false;
  }
}

async function persistSettings() {
  if (isSaving.value) {
    return false;
  }

  isSaving.value = true;

  const persisted = props.settings;
  const btChanged =
    persisted != null &&
    (persisted.bt.dhtEnabled !== form.bt.dhtEnabled ||
      persisted.bt.trackerList !== form.bt.trackerList);

  try {
    const saved = await saveAppSettings(buildSettingsPayload());

    savedSettingsSnapshot.value = serializeSettings(saved);
    emit("saved", saved);
    emit("dirtyChange", false);
    notifySuccess(t("settings.notifications.saved"));

    if (btChanged) {
      notifyInfo(t("settings.notifications.btRestartRequired"), 5000);
    }

    return true;
  } catch (error) {
    notifyError(
      error instanceof Error ? error.message : t("settings.notifications.saveFailed"),
    );
    return false;
  } finally {
    isSaving.value = false;
  }
}

const activeTab = ref("appearance");

const tabs = [
  { id: "appearance", icon: "i-ri-palette-line", labelKey: "settings.appearanceKicker" },
  { id: "scheduler", icon: "i-ri-dashboard-line", labelKey: "settings.scheduler" },
  { id: "downloads", icon: "i-ri-download-line", labelKey: "settings.downloads" },
  { id: "bt", icon: "i-ri-seedling-line", labelKey: "settings.bt" },
  { id: "aria2Rpc", icon: "i-ri-terminal-box-line", labelKey: "settings.aria2Rpc" },
  { id: "logging", icon: "i-ri-file-list-3-line", labelKey: "settings.logging" },
  { id: "proxy", icon: "i-ri-global-line", labelKey: "settings.proxyTitle" },
] as const;

defineExpose({
  persistSettings,
});
</script>

<template>
  <section class="settings-page">

    <NotificationToast
      :notifications="notifications"
      @dismiss="dismiss"
    />

    <div class="desk-panel__header settings-page__header">
      <div>
        <p class="section-kicker">{{ t("settings.kicker") }}</p>
        <h2 class="panel-title">{{ t("settings.title") }}</h2>
      </div>
      <div class="settings-page__header-meta">
        <p class="settings-page__summary">{{ pageSummary }}</p>
      </div>
    </div>

    <div class="settings-tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="settings-tab"
        :class="{ 'settings-tab--active': activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        <span :class="tab.icon" aria-hidden="true" />
        <span>{{ t(tab.labelKey) }}</span>
      </button>
    </div>

    <div class="settings-tab-content">
      <SettingsAppearancePanel
        v-show="activeTab === 'appearance'"
        :draft="form"
        :t="t"
        :language="language"
        :language-options="languageOptions"
        :color-mode-options="colorModeOptions"
        :background-opacity-options="backgroundOpacityOptions"
        @change-language="changeLanguage"
      />

      <SettingsSchedulerPanel
        v-show="activeTab === 'scheduler'"
        :draft="form"
        :t="t"
        :scheduler-mode-options="schedulerModeOptions"
        :adaptive-profile-options="adaptiveProfileOptions"
        :global-speed-limit-mi-bps="globalSpeedLimitMiBps"
        @update:globalSpeedLimitMiBps="setGlobalSpeedLimitMiBps"
      />

      <SettingsDownloadDefaultsPanel
        v-show="activeTab === 'downloads'"
        :draft="form"
        :t="t"
        :checksum-options="checksumOptions"
        :download-summary="downloadSummary"
        :is-picking-directory="isPickingDirectory"
        :default-user-agent-placeholder="DEFAULT_HTTP_USER_AGENT"
        @pick-directory="pickDefaultDownloadDirectory"
      />

      <SettingsBtPanel
        v-show="activeTab === 'bt'"
        :draft="form"
        :t="t"
        :bt-summary="btSummary"
        :bt-upload-limit-mi-b="btUploadLimitMiB"
        :is-fetching-tracker-list="isFetchingTrackerList"
        :default-tracker-list-url="DEFAULT_TRACKER_LIST_URL"
        @update:btUploadLimitMiB="setBtUploadLimitMiB"
        @fetch-tracker-list="updateTrackerListFromUrl"
      />

      <SettingsAria2RpcPanel
        v-show="activeTab === 'aria2Rpc'"
        :draft="form"
        :t="t"
      />

      <SettingsLoggingPanel
        v-show="activeTab === 'logging'"
        :draft="form"
        :t="t"
        :log-level-options="logLevelOptions"
        :logging-summary="loggingSummary"
      />

      <SettingsProxyPanel
        v-show="activeTab === 'proxy'"
        :draft="form"
        :t="t"
        :proxy-mode-options="proxyModeOptions"
        :proxy-summary="proxySummary"
      />

      </div>

    <div class="settings-save-bar">
      <p class="settings-save-bar__hint">{{ t("settings.saveHint") }}</p>
      <UiButton type="button" icon="i-ri-save-line" :loading="isSaving" @click="persistSettings">
        {{ isSaving ? t("common.saving") : t("common.save") }}
      </UiButton>
    </div>
  </section>
</template>

<style scoped>
.settings-page {
  display: grid;
  gap: 1rem;
  padding-bottom: 5.75rem;
}

.settings-page__header {
  align-items: flex-end;
}

.settings-page__header-meta {
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  gap: 1rem;
  min-width: 0;
}

.settings-page__summary {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
  max-width: 40rem;
  text-align: right;
}

.settings-page__summary--secondary {
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  margin-top: var(--space-1);
}

.settings-save-bar {
  position: fixed;
  left: calc(clamp(14.5rem, 18vw, 16rem) + 1.25rem);
  right: 1.25rem;
  bottom: 1.25rem;
  z-index: 30;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) var(--surface-card-alpha), transparent);
  box-shadow: var(--shadow-card-hover);
  backdrop-filter: blur(var(--surface-blur));
}

.settings-save-bar__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.85rem;
  line-height: 1.45;
}

@media (max-width: 960px) {
  .settings-save-bar {
    left: 1rem;
    right: 1rem;
  }

  .settings-page__summary {
    max-width: none;
    text-align: left;
  }

  .settings-page__header-meta {
    width: 100%;
    align-items: flex-start;
    flex-direction: column;
  }
}

@media (max-width: 840px) {
  .settings-save-bar {
    align-items: stretch;
    flex-direction: column;
  }
}

.settings-tabs {
  display: inline-grid;
  grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
  gap: 0.35rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) var(--surface-panel-alpha), transparent);
}

.settings-tab {
  min-height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 0.82rem;
  padding: 0 0.6rem;
  transition:
    background-color 0.18s ease,
    border-color 0.18s ease,
    color 0.18s ease;
}

.settings-tab:hover {
  color: var(--color-heading);
  background: color-mix(in srgb, var(--color-accent-soft) 24%, var(--color-panel));
}

.settings-tab--active {
  color: var(--color-accent-strong);
  border-color: color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent-soft) 52%, var(--color-panel));
}

.settings-tab-content {
  display: grid;
  gap: 1rem;
}

@media (max-width: 960px) {
  .settings-tabs {
    grid-template-columns: repeat(auto-fit, minmax(5.5rem, 1fr));
  }

  .settings-tab {
    font-size: 0.78rem;
    padding: 0 0.35rem;
  }
}

@media (max-width: 680px) {
  .settings-tabs {
    display: flex;
    flex-wrap: nowrap;
    overflow-x: auto;
    scrollbar-width: none;
    -webkit-overflow-scrolling: touch;
  }

  .settings-tabs::-webkit-scrollbar {
    display: none;
  }

  .settings-tab {
    flex-shrink: 0;
    white-space: nowrap;
  }
}
</style>

<style>
/* ── Shared structural classes for settings panels ───────────────── */
/* NON-SCOPED: the 7 child panel components require these classes to render correctly.
   This is a deliberate coupling tradeoff — renaming any class here will silently
   break child panels (SettingsAppearancePanel, SettingsBtPanel, etc.).
   When modifying, search for usages across all settings/*.vue files.            */
/* ────────────────────────────────────────────────────────────────── */

.settings-section {
  display: grid;
  gap: 1rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) var(--surface-card-alpha), transparent);
  box-shadow: var(--shadow-card);
}

.settings-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.settings-section__head h3 {
  margin: 0.2rem 0 0;
  color: var(--color-heading);
  font-size: 1rem;
}

.settings-section__icon {
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 10%, var(--color-panel-muted));
  border: 1px solid color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
}

.settings-section__summary {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
}

.settings-grid {
  display: grid;
  align-items: start;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
}

.settings-field {
  display: grid;
  gap: 0.45rem;
  align-content: start;
  grid-auto-rows: max-content;
  min-width: 0;
}

.settings-field--wide {
  grid-column: 1 / -1;
}

.settings-field__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.settings-field__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  line-height: 1.5;
}

.settings-directory-field,
.settings-inline-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.75rem;
}

.settings-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  min-height: 2.75rem;
  padding: 0 0.9rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--color-panel) var(--surface-panel-alpha), transparent);
  color: var(--color-text-muted);
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    color 0.2s ease;
}

.settings-toggle:hover {
  border-color: var(--color-border-strong);
  background: color-mix(in srgb, var(--color-panel-muted) 72%, var(--color-panel));
}

.settings-toggle:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.settings-toggle--active {
  border-color: color-mix(in srgb, var(--color-accent) 32%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent-soft) 45%, var(--color-panel));
  color: var(--color-accent-strong);
}

.settings-toggle__icon {
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 1.1rem;
}

.settings-toggle__text {
  color: var(--color-heading);
  font-size: 0.9rem;
}

.settings-textarea {
  width: 100%;
  min-height: 8.5rem;
  padding: 0.8rem 0.9375rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  line-height: 1.5;
  resize: vertical;
  transition:
    border-color 0.25s ease,
    box-shadow 0.25s ease,
    background-color 0.25s ease;
}

.settings-textarea::placeholder {
  color: var(--color-text-soft);
}

.settings-textarea:hover:not(:focus-visible) {
  border-color: var(--color-border-strong);
}

.settings-textarea:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.settings-metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.85rem;
}

.settings-metric-card {
  display: grid;
  gap: 0.35rem;
  padding: 0.85rem 0.9rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel-muted) var(--surface-muted-alpha), transparent);
}

.settings-metric-card__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.settings-metric-card__value {
  color: var(--color-heading);
  font-size: 0.95rem;
  line-height: 1.4;
}

@media (max-width: 960px) {
  .settings-grid,
  .settings-metrics-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 840px) {
  .settings-grid,
  .settings-metrics-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-field--wide {
    grid-column: auto;
  }

  .settings-directory-field,
  .settings-inline-field {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
