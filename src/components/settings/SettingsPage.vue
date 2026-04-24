<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";

import { formatBytes, formatSpeed, formatTimestamp } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import { pickDirectory } from "../../lib/tauri/dialog-api";
import { fetchTrackerList, saveAppSettings } from "../../lib/tauri/settings-api";
import type { ChecksumMode } from "../../types/download";
import type { SupportedLanguage } from "../../i18n/resources";
import type {
  AdaptiveProfile,
  AppSettings,
  DeviceLearningMode,
  NetworkLearningSettings,
  NetworkSceneProfile,
  ProxyMode,
  SchedulerMode,
  ThemeColor,
} from "../../types/settings";
import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";

const DEFAULT_TRACKER_LIST_URL = "https://cf.trackerslist.com/best.txt";
const DEFAULT_HTTP_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
  dirtyChange: [isDirty: boolean];
}>();

const { language, languageOptions, setLanguage, t } = useI18n();

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

const form = reactive<AppSettings>({
  appearance: {
    themeColor: "default",
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
      adaptiveProfile: "balanced",
    },
  },
  download: {
    defaultDownloadDir: "",
    defaultMaxRetries: 5,
    defaultChecksum: "blake3",
    defaultUserAgent: DEFAULT_HTTP_USER_AGENT,
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
});

const isSaving = ref(false);
const isPickingDirectory = ref(false);
const isFetchingTrackerList = ref(false);
const notificationMessage = ref("");
const savedSettingsSnapshot = ref("");
let notificationTimer: ReturnType<typeof setTimeout> | null = null;

const currentScene = computed(() => {
  return form.networkLearning.scenes[0] ?? null;
});

const pageSummary = computed(() => {
  if (form.scheduler.mode === "traditional") {
    return t("settings.summaries.traditional", {
      tasks: form.scheduler.traditional.maxParallelTasks,
      deviceMode: deviceModeLabel(form.networkLearning.deviceMode),
    });
  }

  return t("settings.summaries.automatic", {
    threads: form.scheduler.automatic.maxParallelThreads,
    perTask: form.scheduler.automatic.maxThreadsPerTask,
    profile: profileLabel(form.scheduler.automatic.adaptiveProfile),
  });
});

const proxySummary = computed(() => {
  if (form.proxy.mode === "disabled") {
    return t("settings.summaries.proxyDisabled");
  }

  if (form.proxy.mode === "system") {
    return t("settings.summaries.proxySystem");
  }

  return form.proxy.manualUrl.trim()
    ? t("settings.summaries.proxyManual", { url: form.proxy.manualUrl.trim() })
    : t("settings.summaries.proxyManualEmpty");
});

const downloadSummary = computed(() => {
  const location = form.download.defaultDownloadDir.trim() || t("settings.unsetDefaultPath");
  const checksumLabel =
    checksumOptions.value.find((option) => option.value === form.download.defaultChecksum)?.label ??
    form.download.defaultChecksum;

  return t("settings.summaries.download", {
    location,
    retries: form.download.defaultMaxRetries,
    checksum: checksumLabel,
    userAgent: form.download.defaultUserAgent.trim() || DEFAULT_HTTP_USER_AGENT,
  });
});

const btUploadLimitMiB = computed({
  get() {
    return Math.round(form.bt.uploadLimitBytes / 1024 / 1024);
  },
  set(value: number | null) {
    form.bt.uploadLimitBytes = Math.max(0, Math.trunc(value ?? 0)) * 1024 * 1024;
  },
});

const btSummary = computed(() => {
  const dhtLabel = form.bt.dhtEnabled ? t("common.enabled") : t("common.disabled");
  const pexLabel = form.bt.pexEnabled ? t("common.enabled") : t("common.disabled");
  const trackerCount = trackerListEntries.value.length;

  if (!form.bt.pauseUploadWhenLimitReached) {
    return t("settings.summaries.btDisabled", {
      dht: dhtLabel,
      pex: pexLabel,
      trackers: trackerCount,
    });
  }

  const uploadLimit =
    form.bt.uploadLimitBytes > 0 ? formatBytes(form.bt.uploadLimitBytes) : t("common.disabled");
  const ratioLimit =
    form.bt.uploadRatioLimit > 0 ? `${form.bt.uploadRatioLimit.toFixed(2)}x` : t("common.disabled");

  return t("settings.summaries.bt", {
    dht: dhtLabel,
    pex: pexLabel,
    trackers: trackerCount,
    uploadLimit,
    ratioLimit,
  });
});

const trackerListEntries = computed(() =>
  form.bt.trackerList
    .split(/\r?\n/)
    .map((tracker) => tracker.trim())
    .filter(Boolean),
);

const settingsDraftSnapshot = computed(() => serializeSettings(buildSettingsPayload()));

const networkLearningSummary = computed(() => {
  const scene = currentScene.value;
  if (!scene) {
    return t("settings.summaries.noNetworkProfile");
  }

  if (form.networkLearning.deviceMode === "mobile") {
    return t("settings.summaries.mobile", {
      deviceMode: deviceModeLabel(form.networkLearning.deviceMode),
    });
  }

  if (!scene.learningEnabled) {
    return t("settings.summaries.learningPaused");
  }

  if (!scene.learnedMetrics) {
    return t("settings.summaries.noLearningSamples");
  }

  return t("settings.summaries.learning", {
    deviceMode: deviceModeLabel(form.networkLearning.deviceMode),
    samples: scene.learnedMetrics.sampleCount,
    threads: scene.learnedMetrics.recommendedInitialThreads,
  });
});

const networkMetricsCards = computed(() => {
  const metrics = currentScene.value?.learnedMetrics;
  const learningOpen =
    form.networkLearning.deviceMode !== "mobile" && Boolean(currentScene.value?.learningEnabled);

  return [
    {
      label: t("settings.metrics.learningStatus"),
      value: learningOpen ? t("common.enabled") : t("common.disabled"),
    },
    {
      label: t("settings.metrics.estimatedBandwidth"),
      value: metrics ? formatSpeed(metrics.estimatedBandwidthBps) : t("common.dash"),
    },
    {
      label: t("settings.metrics.stability"),
      value: metrics ? stabilityLabel(metrics.stabilityScore) : t("common.dash"),
    },
    {
      label: t("settings.metrics.penaltyRate"),
      value: metrics ? formatPercent(metrics.penaltyRate) : t("common.dash"),
    },
    {
      label: t("settings.metrics.recommendedInitialThreads"),
      value: metrics ? String(metrics.recommendedInitialThreads) : t("common.dash"),
    },
    {
      label: t("settings.metrics.recommendedThreadCap"),
      value: metrics ? String(metrics.recommendedMaxThreadsPerTaskCap) : t("common.dash"),
    },
    {
      label: t("settings.metrics.sampleCount"),
      value: metrics ? String(metrics.sampleCount) : "0",
    },
    {
      label: t("settings.metrics.lastLearnedAt"),
      value: metrics ? formatTimestamp(metrics.lastObservedAtMs) : t("common.dash"),
    },
  ];
});

watch(
  () => props.settings,
  (nextSettings) => {
    if (!nextSettings) {
      return;
    }

    form.appearance.themeColor = nextSettings.appearance?.themeColor ?? "default";
    form.proxy.mode = nextSettings.proxy.mode;
    form.proxy.manualUrl = nextSettings.proxy.manualUrl;
    form.scheduler.mode = nextSettings.scheduler.mode;
    form.scheduler.traditional.maxParallelTasks =
      nextSettings.scheduler.traditional.maxParallelTasks;
    form.scheduler.automatic.maxParallelThreads =
      nextSettings.scheduler.automatic.maxParallelThreads;
    form.scheduler.automatic.maxThreadsPerTask = nextSettings.scheduler.automatic.maxThreadsPerTask;
    form.scheduler.automatic.adaptiveProfile = nextSettings.scheduler.automatic.adaptiveProfile;
    form.download.defaultDownloadDir = nextSettings.download.defaultDownloadDir;
    form.download.defaultMaxRetries = nextSettings.download.defaultMaxRetries;
    form.download.defaultChecksum = nextSettings.download.defaultChecksum;
    form.download.defaultUserAgent = nextSettings.download.defaultUserAgent || DEFAULT_HTTP_USER_AGENT;
    form.bt.dhtEnabled = nextSettings.bt.dhtEnabled;
    form.bt.pexEnabled = nextSettings.bt.pexEnabled;
    form.bt.trackerList = nextSettings.bt.trackerList;
    form.bt.trackerListUrl = nextSettings.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL;
    form.bt.pauseUploadWhenLimitReached = nextSettings.bt.pauseUploadWhenLimitReached;
    form.bt.uploadLimitBytes = nextSettings.bt.uploadLimitBytes;
    form.bt.uploadRatioLimit = nextSettings.bt.uploadRatioLimit;
    form.networkLearning.deviceMode = nextSettings.networkLearning.deviceMode;
    form.networkLearning.currentSceneId = "default";
    form.networkLearning.scenes = [copySingleNetworkScene(nextSettings.networkLearning)];
    savedSettingsSnapshot.value = serializeSettings(buildSettingsPayload());
    emit("dirtyChange", false);
  },
  { immediate: true },
);

watch(
  settingsDraftSnapshot,
  (snapshot) => {
    if (!savedSettingsSnapshot.value) {
      return;
    }

    emit("dirtyChange", snapshot !== savedSettingsSnapshot.value);
  },
  { immediate: true },
);

watch(
  () => form.scheduler.automatic.maxParallelThreads,
  (value) => {
    if (form.scheduler.automatic.maxThreadsPerTask > value) {
      form.scheduler.automatic.maxThreadsPerTask = value;
    }
  },
);

function profileLabel(profile: AdaptiveProfile) {
  return adaptiveProfileOptions.value.find((option) => option.value === profile)?.label ?? profile;
}

function deviceModeLabel(mode: DeviceLearningMode) {
  return deviceModeOptions.value.find((option) => option.value === mode)?.label ?? mode;
}

function formatPercent(value: number) {
  return `${(value * 100).toFixed(value >= 0.1 ? 0 : 1)}%`;
}

function stabilityLabel(score: number) {
  if (score >= 0.88) {
    return t("settings.metrics.high");
  }
  if (score >= 0.68) {
    return t("settings.metrics.medium");
  }
  return t("settings.metrics.low");
}

function showNotification(message: string) {
  notificationMessage.value = message;
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
  notificationTimer = setTimeout(() => {
    notificationMessage.value = "";
    notificationTimer = null;
  }, 2200);
}

function changeLanguage(nextLanguage: SupportedLanguage) {
  void setLanguage(nextLanguage);
}

function copySingleNetworkScene(settings: NetworkLearningSettings): NetworkSceneProfile {
  const selectedScene =
    settings.scenes.find((scene) => scene.id === settings.currentSceneId) ?? settings.scenes[0];
  return {
    id: "default",
    name: t("settings.defaultScene"),
    learningEnabled: selectedScene?.learningEnabled ?? true,
    learnedMetrics: selectedScene?.learnedMetrics ? { ...selectedScene.learnedMetrics } : null,
    updatedAtMs: selectedScene?.updatedAtMs ?? 0,
  };
}

function buildSettingsPayload(): AppSettings {
  return {
    appearance: {
      themeColor: form.appearance.themeColor,
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
        adaptiveProfile: form.scheduler.automatic.adaptiveProfile,
      },
    },
    download: {
      defaultDownloadDir: form.download.defaultDownloadDir,
      defaultMaxRetries: form.download.defaultMaxRetries,
      defaultChecksum: form.download.defaultChecksum,
      defaultUserAgent: form.download.defaultUserAgent,
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
      scenes: [copySingleNetworkScene(form.networkLearning)],
    },
  };
}

function serializeSettings(settings: AppSettings) {
  return JSON.stringify(settings);
}

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
    showNotification(
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
    form.bt.trackerList = await fetchTrackerList(form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL);
    showNotification(
      t("settings.notifications.trackerListUpdated", {
        count: trackerListEntries.value.length,
      }),
    );
  } catch (error) {
    showNotification(
      error instanceof Error ? error.message : t("settings.notifications.trackerListUpdateFailed"),
    );
  } finally {
    isFetchingTrackerList.value = false;
  }
}

async function persistSettings() {
  if (isSaving.value) {
    return;
  }

  isSaving.value = true;

  try {
    const saved = await saveAppSettings(buildSettingsPayload());

    savedSettingsSnapshot.value = serializeSettings(saved);
    emit("saved", saved);
    emit("dirtyChange", false);
    showNotification(t("settings.notifications.saved"));
  } catch (error) {
    showNotification(
      error instanceof Error ? error.message : t("settings.notifications.saveFailed"),
    );
  } finally {
    isSaving.value = false;
  }
}

onBeforeUnmount(() => {
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
});
</script>

<template>
  <section class="settings-page">
    <Transition name="settings-notification">
      <div v-if="notificationMessage" class="settings-notification" role="status">
        <span class="i-ri-checkbox-circle-line" aria-hidden="true" />
        <span>{{ notificationMessage }}</span>
      </div>
    </Transition>

    <div class="desk-panel__header settings-page__header">
      <div>
        <p class="section-kicker">{{ t("settings.kicker") }}</p>
        <h2 class="panel-title">{{ t("settings.title") }}</h2>
      </div>
      <div class="settings-page__header-meta">
        <p class="settings-page__summary">{{ pageSummary }}</p>
        <UiButton
          type="button"
          variant="secondary"
          icon="i-ri-save-line"
          :disabled="isSaving"
          @click="persistSettings"
        >
          {{ isSaving ? t("common.saving") : t("common.save") }}
        </UiButton>
      </div>
    </div>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("language.label") }}</p>
          <h3>{{ t("settings.languageTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-translate-2" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("language.label") }}</span>
          <UiSelect
            :model-value="language"
            :options="languageOptions"
            @update:model-value="changeLanguage($event as SupportedLanguage)"
          />
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.appearanceKicker") }}</p>
          <h3>{{ t("settings.appearanceTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-palette-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.themeColor") }}</span>
          <div class="theme-color-options">
            <button
              v-for="color in ['default', 'amber', 'sky', 'lime']"
              :key="color"
              type="button"
              class="theme-color-button"
              :class="['theme-color-button--' + color, { 'is-active': form.appearance.themeColor === color }]"
              :aria-label="t(`settings.themeColorNames.${color}`)"
              @click="form.appearance.themeColor = color as ThemeColor"
            >
              <span v-if="form.appearance.themeColor === color" class="i-ri-check-line" aria-hidden="true" />
            </button>
          </div>
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.scheduler") }}</p>
          <h3>{{ t("settings.schedulerTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-git-branch-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.allocationMode") }}</span>
          <UiSelect v-model="form.scheduler.mode" :options="schedulerModeOptions" />
        </label>

        <label v-if="form.scheduler.mode === 'traditional'" class="settings-field">
          <span class="settings-field__label">{{ t("settings.maxParallelTasks") }}</span>
          <UiNumberField v-model="form.scheduler.traditional.maxParallelTasks" :min="1" :max="32" />
          <p class="settings-field__hint">{{ t("settings.traditionalHint") }}</p>
        </label>

        <template v-else>
          <label class="settings-field">
            <span class="settings-field__label">{{ t("settings.maxParallelThreads") }}</span>
            <UiNumberField
              v-model="form.scheduler.automatic.maxParallelThreads"
              :min="1"
              :max="64"
            />
          </label>

          <label class="settings-field">
            <span class="settings-field__label">{{ t("settings.maxThreadsPerTask") }}</span>
            <UiNumberField
              v-model="form.scheduler.automatic.maxThreadsPerTask"
              :min="1"
              :max="Math.max(1, form.scheduler.automatic.maxParallelThreads)"
            />
          </label>

          <label class="settings-field settings-field--wide">
            <span class="settings-field__label">{{ t("settings.adaptiveProfile") }}</span>
            <UiSelect
              v-model="form.scheduler.automatic.adaptiveProfile"
              :options="adaptiveProfileOptions"
            />
            <p class="settings-field__hint">
              {{ t("settings.adaptiveProfileHint") }}
            </p>
          </label>
        </template>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.networkLearning") }}</p>
          <h3>{{ t("settings.networkLearningTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-radar-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ networkLearningSummary }}</p>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.deviceMode") }}</span>
          <UiSelect v-model="form.networkLearning.deviceMode" :options="deviceModeOptions" />
          <p class="settings-field__hint">
            {{ t("settings.deviceModeHint") }}
          </p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.allowLearning") }}</span>
          <button
            v-if="currentScene"
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': currentScene.learningEnabled }"
            :aria-pressed="currentScene.learningEnabled"
            @click="currentScene.learningEnabled = !currentScene.learningEnabled"
          >
            <span
              class="settings-toggle__icon"
              :class="
                currentScene.learningEnabled
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">
              {{
                currentScene?.learningEnabled
                  ? t("settings.allowUpdateProfile")
                  : t("settings.pauseUpdateProfile")
              }}
            </span>
          </button>
        </label>
      </div>

      <div class="settings-metrics-grid">
        <article v-for="item in networkMetricsCards" :key="item.label" class="settings-metric-card">
          <span class="settings-metric-card__label">{{ item.label }}</span>
          <strong class="settings-metric-card__value">{{ item.value }}</strong>
        </article>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.downloads") }}</p>
          <h3>{{ t("settings.downloadsTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-download-2-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ downloadSummary }}</p>

      <div class="settings-grid">
        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.defaultDownloadLocation") }}</span>
          <div class="settings-directory-field">
            <UiInput
              v-model="form.download.defaultDownloadDir"
              type="text"
              :placeholder="t('settings.defaultDownloadPlaceholder')"
            />
            <UiButton
              type="button"
              variant="secondary"
              size="sm"
              :loading="isPickingDirectory"
              @click="pickDefaultDownloadDirectory"
            >
              {{ isPickingDirectory ? t("common.browsing") : t("common.browse") }}
            </UiButton>
          </div>
          <p class="settings-field__hint">
            {{ t("settings.defaultDownloadHint") }}
          </p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.defaultRetries") }}</span>
          <UiNumberField v-model="form.download.defaultMaxRetries" :min="0" :max="20" />
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.globalChecksum") }}</span>
          <UiSelect v-model="form.download.defaultChecksum" :options="checksumOptions" />
          <p class="settings-field__hint">{{ t("settings.checksumHint") }}</p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.defaultUserAgent") }}</span>
          <UiInput
            v-model="form.download.defaultUserAgent"
            type="text"
            :placeholder="DEFAULT_HTTP_USER_AGENT"
          />
          <p class="settings-field__hint">{{ t("settings.defaultUserAgentHint") }}</p>
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.bt") }}</p>
          <h3>{{ t("settings.btTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-seedling-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ btSummary }}</p>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.btDht") }}</span>
          <button
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': form.bt.dhtEnabled }"
            :aria-pressed="form.bt.dhtEnabled"
            @click="form.bt.dhtEnabled = !form.bt.dhtEnabled"
          >
            <span
              class="settings-toggle__icon"
              :class="
                form.bt.dhtEnabled
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">
              {{ form.bt.dhtEnabled ? t("settings.btDhtEnabled") : t("settings.btDhtDisabled") }}
            </span>
          </button>
          <p class="settings-field__hint">{{ t("settings.btDhtHint") }}</p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.btPex") }}</span>
          <button
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': form.bt.pexEnabled }"
            :aria-pressed="form.bt.pexEnabled"
            @click="form.bt.pexEnabled = !form.bt.pexEnabled"
          >
            <span
              class="settings-toggle__icon"
              :class="
                form.bt.pexEnabled
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">
              {{ form.bt.pexEnabled ? t("settings.btPexEnabled") : t("settings.btPexDisabled") }}
            </span>
          </button>
          <p class="settings-field__hint">{{ t("settings.btPexHint") }}</p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.btTrackerListUrl") }}</span>
          <div class="settings-inline-field">
            <UiInput
              v-model="form.bt.trackerListUrl"
              type="url"
              inputmode="url"
              :placeholder="DEFAULT_TRACKER_LIST_URL"
            />
            <UiButton
              type="button"
              variant="secondary"
              size="sm"
              icon="i-ri-refresh-line"
              :loading="isFetchingTrackerList"
              @click="updateTrackerListFromUrl"
            >
              {{
                isFetchingTrackerList
                  ? t("settings.btTrackerListUpdating")
                  : t("settings.btTrackerListUpdate")
              }}
            </UiButton>
          </div>
          <p class="settings-field__hint">{{ t("settings.btTrackerListUrlHint") }}</p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.btTrackerList") }}</span>
          <textarea
            v-model="form.bt.trackerList"
            class="settings-textarea"
            :placeholder="t('settings.btTrackerListPlaceholder')"
            rows="5"
            spellcheck="false"
          />
          <p class="settings-field__hint">
            {{ t("settings.btTrackerListHint", { count: trackerListEntries.length }) }}
          </p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.btPauseUpload") }}</span>
          <button
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': form.bt.pauseUploadWhenLimitReached }"
            :aria-pressed="form.bt.pauseUploadWhenLimitReached"
            @click="form.bt.pauseUploadWhenLimitReached = !form.bt.pauseUploadWhenLimitReached"
          >
            <span
              class="settings-toggle__icon"
              :class="
                form.bt.pauseUploadWhenLimitReached
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">
              {{
                form.bt.pauseUploadWhenLimitReached
                  ? t("settings.btPauseUploadEnabled")
                  : t("settings.btPauseUploadDisabled")
              }}
            </span>
          </button>
          <p class="settings-field__hint">{{ t("settings.btPauseUploadHint") }}</p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.btUploadLimit") }}</span>
          <UiNumberField
            v-model="btUploadLimitMiB"
            :min="0"
            :max="10485760"
            :disabled="!form.bt.pauseUploadWhenLimitReached"
          />
          <p class="settings-field__hint">{{ t("settings.btUploadLimitHint") }}</p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.btRatioLimit") }}</span>
          <UiNumberField
            v-model="form.bt.uploadRatioLimit"
            :min="0"
            :max="100"
            :step="0.1"
            :disabled="!form.bt.pauseUploadWhenLimitReached"
          />
          <p class="settings-field__hint">{{ t("settings.btRatioLimitHint") }}</p>
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.network") }}</p>
          <h3>{{ t("settings.proxyTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-global-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ proxySummary }}</p>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.proxyMode") }}</span>
          <UiSelect v-model="form.proxy.mode" :options="proxyModeOptions" />
        </label>

        <label v-if="form.proxy.mode === 'manual'" class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.proxyAddress") }}</span>
          <UiInput v-model="form.proxy.manualUrl" type="text" placeholder="http://127.0.0.1:7890" />
          <p class="settings-field__hint">
            {{ t("settings.proxyHint") }}
          </p>
        </label>
      </div>
    </section>
  </section>
</template>

<style scoped>
.settings-page {
  display: grid;
  gap: 1rem;
}

.settings-notification {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 40;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 0.9rem;
  border: 1px solid var(--color-success-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) 96%, transparent);
  box-shadow: var(--shadow-card-hover);
  color: var(--color-success-text);
  font-size: 0.85rem;
  backdrop-filter: blur(0.875rem);
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

.settings-page__summary,
.settings-section__summary {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
}

.settings-page__summary {
  max-width: 40rem;
  text-align: right;
}

.settings-section {
  display: grid;
  gap: 1rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) 94%, transparent);
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
  background: color-mix(in srgb, var(--color-panel) 92%, transparent);
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
  background: color-mix(in srgb, var(--color-panel-muted) 78%, transparent);
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

.settings-notification-enter-active,
.settings-notification-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.settings-notification-enter-from,
.settings-notification-leave-to {
  opacity: 0;
  transform: translateY(-0.45rem);
}

@media (max-width: 960px) {
  .settings-page__summary {
    max-width: none;
    text-align: left;
  }

  .settings-page__header-meta {
    width: 100%;
    align-items: flex-start;
    flex-direction: column;
  }

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
