<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";

import { formatSpeed, formatTimestamp } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import { pickDirectory } from "../../lib/tauri/dialog-api";
import { saveAppSettings } from "../../lib/tauri/settings-api";
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
} from "../../types/settings";
import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
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
const notificationMessage = ref("");
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
  });
});

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
    form.networkLearning.deviceMode = nextSettings.networkLearning.deviceMode;
    form.networkLearning.currentSceneId = "default";
    form.networkLearning.scenes = [copySingleNetworkScene(nextSettings.networkLearning)];
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

async function persistSettings() {
  if (isSaving.value) {
    return;
  }

  isSaving.value = true;

  try {
    const saved = await saveAppSettings({
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
      },
      networkLearning: {
        deviceMode: form.networkLearning.deviceMode,
        currentSceneId: "default",
        scenes: [copySingleNetworkScene(form.networkLearning)],
      },
    });

    emit("saved", saved);
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
      <p class="settings-page__summary">{{ pageSummary }}</p>
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

      <div class="settings-actions">
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
          <span class="settings-toggle">
            <input
              v-if="currentScene"
              v-model="currentScene.learningEnabled"
              class="settings-toggle__control"
              type="checkbox"
            />
            <span class="settings-toggle__text">
              {{
                currentScene?.learningEnabled
                  ? t("settings.allowUpdateProfile")
                  : t("settings.pauseUpdateProfile")
              }}
            </span>
          </span>
        </label>
      </div>

      <div class="settings-metrics-grid">
        <article v-for="item in networkMetricsCards" :key="item.label" class="settings-metric-card">
          <span class="settings-metric-card__label">{{ item.label }}</span>
          <strong class="settings-metric-card__value">{{ item.value }}</strong>
        </article>
      </div>

      <div class="settings-actions">
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
      </div>

      <div class="settings-actions">
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

      <div class="settings-actions">
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

.settings-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--color-border);
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
}

.settings-toggle__control {
  width: 1rem;
  height: 1rem;
  accent-color: var(--color-accent);
}

.settings-toggle__text {
  color: var(--color-heading);
  font-size: 0.9rem;
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

  .settings-actions {
    align-items: flex-start;
    flex-direction: column;
  }

  .settings-directory-field,
  .settings-inline-field {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
