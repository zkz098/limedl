<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import { useI18n } from "../../i18n";
import { useNotification } from "../../composables/useNotification";
import { saveAppSettings } from "../../lib/tauri/settings-api";
import type {
  AdaptiveProfile,
  AppSettings,
  DeviceLearningMode,
  LogLevel,
  NetworkLearningSettings,
} from "../../types/settings";
import type { ChecksumMode } from "../../types/download";
import UiButton from "../ui/UiButton.vue";

import LabsCdnAccelerationPanel from "./LabsCdnAccelerationPanel.vue";
import LabsNetworkLearningPanel from "./LabsNetworkLearningPanel.vue";

import {
  copySingleNetworkScene,
  DEFAULT_HTTP_USER_AGENT,
  DEFAULT_TRACKER_LIST_URL,
  serializeSettings,
  settingsDraftSnapshot,
  useSettingsSummaries,
  type SettingsOptionArrays,
} from "../settings/settingsComposables";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
  dirtyChange: [isDirty: boolean];
}>();

const { t } = useI18n();
const { notifySuccess, notifyError } = useNotification();

// ── Option arrays ────────────────────────────────────────────────

const deviceModeOptions = computed<Array<{ label: string; value: DeviceLearningMode }>>(() => [
  { label: t("tokens.fixed"), value: "fixed" },
  { label: t("tokens.mobile"), value: "mobile" },
  { label: t("tokens.semi_mobile"), value: "semi_mobile" },
]);

// Placeholder refs — required by SettingsOptionArrays shape, unused by labs panels.
const adaptiveProfileOptions = computed<Array<{ label: string; value: AdaptiveProfile }>>(() => []);
const checksumOptions = computed<Array<{ label: string; value: ChecksumMode }>>(() => []);
const logLevelOptions = computed<Array<{ label: string; value: LogLevel }>>(() => []);

// ── Reactive form (full AppSettings; only CDN + networkLearning are edited here) ──────────

const form = reactive<AppSettings>({
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
    pexEnabled: true,
    trackerList: "",
    trackerListUrl: DEFAULT_TRACKER_LIST_URL,
    pauseUploadWhenLimitReached: false,
    uploadLimitBytes: 0,
    uploadRatioLimit: 0,
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

// ── State ────────────────────────────────────────────────────────

const isSaving = ref(false);
const savedSettingsSnapshot = ref("");

// ── Summaries ────────────────────────────────────────────────────

const optionArrays: SettingsOptionArrays = {
  adaptiveProfileOptions,
  deviceModeOptions,
  checksumOptions,
  logLevelOptions,
};

const { networkLearningSummary, networkMetricsCards } = useSettingsSummaries(form, t, optionArrays);

const settingsDraftSnapshotComputed = computed(() => settingsDraftSnapshot(buildSettingsPayload()));

// ── Settings sync watcher ─────────────────────────────────────────

watch(
  () => props.settings,
  (nextSettings) => {
    if (!nextSettings) {
      return;
    }

    // CDN + network learning: actively edited here.
    form.networkLearning.deviceMode = nextSettings.networkLearning.deviceMode;
    form.networkLearning.currentSceneId = "default";
    form.networkLearning.scenes = [copyNetworkScene(nextSettings.networkLearning)];
    form.cdnAcceleration = { ...nextSettings.cdnAcceleration };

    // Carry through everything else verbatim so saveAppSettings won't reset it.
    form.appearance.themeColor = nextSettings.appearance?.themeColor ?? "default";
    form.appearance.backgroundOpacity = nextSettings.appearance?.backgroundOpacity ?? "default";
    form.appearance.colorMode = nextSettings.appearance?.colorMode ?? "system";
    form.appearance.showDetailInfo = nextSettings.appearance?.showDetailInfo ?? true;
    form.appearance.showHeatmap = nextSettings.appearance?.showHeatmap ?? true;
    form.proxy.mode = nextSettings.proxy.mode;
    form.proxy.manualUrl = nextSettings.proxy.manualUrl;
    form.scheduler.mode = nextSettings.scheduler.mode;
    form.scheduler.traditional.maxParallelTasks = nextSettings.scheduler.traditional.maxParallelTasks;
    form.scheduler.automatic.maxParallelThreads = nextSettings.scheduler.automatic.maxParallelThreads;
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
    form.bt.pexEnabled = nextSettings.bt.pexEnabled;
    form.bt.trackerList = nextSettings.bt.trackerList;
    form.bt.trackerListUrl = nextSettings.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL;
    form.bt.pauseUploadWhenLimitReached = nextSettings.bt.pauseUploadWhenLimitReached;
    form.bt.uploadLimitBytes = nextSettings.bt.uploadLimitBytes;
    form.bt.uploadRatioLimit = nextSettings.bt.uploadRatioLimit;
    form.logging.enabled = nextSettings.logging?.enabled ?? true;
    form.logging.level = nextSettings.logging?.level ?? "info";
    form.logging.filePath = nextSettings.logging?.filePath ?? "";
    form.aria2Rpc.enabled = nextSettings.aria2Rpc?.enabled ?? true;
    form.aria2Rpc.port = nextSettings.aria2Rpc?.port ?? 6800;
    form.aria2Rpc.secret = nextSettings.aria2Rpc?.secret ?? null;

    savedSettingsSnapshot.value = serializeSettings(buildSettingsPayload());
    emit("dirtyChange", false);
  },
  { immediate: true },
);

// ── Dirty tracking ──────────────────────────────────────────────────

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

// ── Helpers ───────────────────────────────────────────────────────

function copyNetworkScene(settings: NetworkLearningSettings) {
  return copySingleNetworkScene(settings, t);
}

function buildSettingsPayload(): AppSettings {
  return {
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
      pexEnabled: form.bt.pexEnabled,
      trackerList: form.bt.trackerList,
      trackerListUrl: form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL,
      pauseUploadWhenLimitReached: form.bt.pauseUploadWhenLimitReached,
      uploadLimitBytes: form.bt.uploadLimitBytes,
      uploadRatioLimit: form.bt.uploadRatioLimit,
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

// ── Actions ────────────────────────────────────────────────────────

async function persistSettings(): Promise<boolean> {
  if (isSaving.value) {
    return false;
  }

  isSaving.value = true;

  try {
    const saved = await saveAppSettings(buildSettingsPayload());

    savedSettingsSnapshot.value = serializeSettings(saved);
    emit("saved", saved);
    emit("dirtyChange", false);
    notifySuccess(t("settings.notifications.saved"));
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

// ── Tabs ──────────────────────────────────────────────────────────

const activeTab = ref("networkLearning");

const tabs = [
  { id: "networkLearning", icon: "i-ri-radar-line", labelKey: "settings.networkLearning" },
  { id: "cdnAcceleration", icon: "i-ri-speed-up-line", labelKey: "settings.cdnAcceleration.title" },
] as const;

defineExpose({
  persistSettings,
});
</script>

<template>
  <section class="labs-page">

    <div class="desk-panel__header labs-page__header">
      <div>
        <p class="section-kicker">{{ t("labs.kicker") }}</p>
        <h2 class="panel-title">{{ t("labs.title") }}</h2>
      </div>
    </div>

    <div class="labs-tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="labs-tab"
        :class="{ 'labs-tab--active': activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        <span :class="tab.icon" aria-hidden="true" />
        <span>{{ t(tab.labelKey) }}</span>
      </button>
    </div>

    <div class="labs-tab-content">
      <LabsNetworkLearningPanel
        v-show="activeTab === 'networkLearning'"
        :draft="form"
        :t="t"
        :device-mode-options="deviceModeOptions"
        :network-learning-summary="networkLearningSummary"
        :network-metrics-cards="networkMetricsCards"
      />

      <LabsCdnAccelerationPanel
        v-show="activeTab === 'cdnAcceleration'"
        :draft="form"
        :t="t"
      />
    </div>

    <div class="labs-save-bar">
      <p class="labs-save-bar__hint">{{ t("settings.saveHint") }}</p>
      <UiButton type="button" icon="i-ri-save-line" :loading="isSaving" @click="persistSettings">
        {{ isSaving ? t("common.saving") : t("common.save") }}
      </UiButton>
    </div>
  </section>
</template>

<style scoped>
.labs-page {
  display: grid;
  gap: 1rem;
  padding-bottom: 5.75rem;
}

.labs-page__header {
  align-items: flex-end;
}

.labs-tabs {
  display: inline-grid;
  grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
  gap: 0.35rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) var(--surface-panel-alpha), transparent);
}

.labs-tab {
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

.labs-tab:hover {
  color: var(--color-heading);
  background: color-mix(in srgb, var(--color-accent-soft) 24%, var(--color-panel));
}

.labs-tab--active {
  color: var(--color-accent-strong);
  border-color: color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent-soft) 52%, var(--color-panel));
}

.labs-tab-content {
  display: grid;
  gap: 1rem;
}

.labs-save-bar {
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

.labs-save-bar__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.85rem;
  line-height: 1.45;
}

@media (max-width: 960px) {
  .labs-tabs {
    grid-template-columns: repeat(auto-fit, minmax(5.5rem, 1fr));
  }

  .labs-tab {
    font-size: 0.78rem;
    padding: 0 0.35rem;
  }

  .labs-save-bar {
    left: 1rem;
    right: 1rem;
  }
}

@media (max-width: 680px) {
  .labs-tabs {
    display: flex;
    flex-wrap: nowrap;
    overflow-x: auto;
    scrollbar-width: none;
    -webkit-overflow-scrolling: touch;
  }

  .labs-tabs::-webkit-scrollbar {
    display: none;
  }

  .labs-tab {
    flex-shrink: 0;
    white-space: nowrap;
  }
}

@media (max-width: 840px) {
  .labs-save-bar {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>

<style>
/* Shared structural classes needed by the labs panels (mirrors SettingsPage.vue).
   Non-scoped on purpose — child panel components reference these class names.
   Modifying these will silently break LabsNetworkLearningPanel.vue.        */

.labs .settings-section,
.labs-page .settings-section {
  display: grid;
  gap: 1rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) var(--surface-card-alpha), transparent);
  box-shadow: var(--shadow-card);
}

.labs .settings-section__head,
.labs-page .settings-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.labs .settings-section__head h3,
.labs-page .settings-section__head h3 {
  margin: 0.2rem 0 0;
  color: var(--color-heading);
  font-size: 1rem;
}

.labs .settings-section__icon,
.labs-page .settings-section__icon {
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

.labs .settings-section__summary,
.labs-page .settings-section__summary {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
}

.labs .settings-grid,
.labs-page .settings-grid {
  display: grid;
  align-items: start;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
}

.labs .settings-field,
.labs-page .settings-field {
  display: grid;
  gap: 0.45rem;
  align-content: start;
  grid-auto-rows: max-content;
  min-width: 0;
}

.labs .settings-field--wide,
.labs-page .settings-field--wide {
  grid-column: 1 / -1;
}

.labs .settings-field__label,
.labs-page .settings-field__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.labs .settings-field__hint,
.labs-page .settings-field__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  line-height: 1.5;
}

.labs .settings-toggle,
.labs-page .settings-toggle {
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

.labs .settings-toggle:hover,
.labs-page .settings-toggle:hover {
  border-color: var(--color-border-strong);
  background: color-mix(in srgb, var(--color-panel-muted) 72%, var(--color-panel));
}

.labs .settings-toggle:focus-visible,
.labs-page .settings-toggle:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.labs .settings-toggle--active,
.labs-page .settings-toggle--active {
  border-color: color-mix(in srgb, var(--color-accent) 32%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent-soft) 45%, var(--color-panel));
  color: var(--color-accent-strong);
}

.labs .settings-toggle__icon,
.labs-page .settings-toggle__icon {
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 1.1rem;
}

.labs .settings-toggle__text,
.labs-page .settings-toggle__text {
  color: var(--color-heading);
  font-size: 0.9rem;
}

.labs .settings-metrics-grid,
.labs-page .settings-metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.85rem;
}

.labs .settings-metric-card,
.labs-page .settings-metric-card {
  display: grid;
  gap: 0.35rem;
  padding: 0.85rem 0.9rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel-muted) var(--surface-muted-alpha), transparent);
}

.labs .settings-metric-card__label,
.labs-page .settings-metric-card__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.labs .settings-metric-card__value,
.labs-page .settings-metric-card__value {
  color: var(--color-heading);
  font-size: 0.95rem;
  line-height: 1.4;
}

@media (max-width: 960px) {
  .labs .settings-grid,
  .labs-page .settings-grid,
  .labs .settings-metrics-grid,
  .labs-page .settings-metrics-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 840px) {
  .labs .settings-grid,
  .labs-page .settings-grid,
  .labs .settings-metrics-grid,
  .labs-page .settings-metrics-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .labs .settings-field--wide,
  .labs-page .settings-field--wide {
    grid-column: auto;
  }
}
</style>