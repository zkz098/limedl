<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";

import { formatSpeed, formatTimestamp } from "../../lib/download-format";
import { pickDirectory } from "../../lib/tauri/dialog-api";
import { saveAppSettings } from "../../lib/tauri/settings-api";
import type { ChecksumMode } from "../../types/download";
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

const proxyModeOptions: Array<{ label: string; value: ProxyMode }> = [
  { label: "不使用代理", value: "disabled" },
  { label: "系统代理", value: "system" },
  { label: "手动设置代理", value: "manual" },
];

const schedulerModeOptions: Array<{ label: string; value: SchedulerMode }> = [
  { label: "自动", value: "automatic" },
  { label: "传统", value: "traditional" },
];

const adaptiveProfileOptions: Array<{ label: string; value: AdaptiveProfile }> = [
  { label: "保守", value: "conservative" },
  { label: "平衡", value: "balanced" },
  { label: "激进", value: "aggressive" },
];

const checksumOptions: Array<{ label: string; value: ChecksumMode }> = [
  { label: "BLAKE3", value: "blake3" },
  { label: "SHA-256", value: "sha256" },
  { label: "XXH3-128", value: "xxh3_128" },
  { label: "None", value: "none" },
];

const deviceModeOptions: Array<{ label: string; value: DeviceLearningMode }> = [
  { label: "固定使用", value: "fixed" },
  { label: "移动使用", value: "mobile" },
  { label: "半移动使用", value: "semi_mobile" },
];

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
        name: "默认场景",
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
    return `传统模式下最多同时运行 ${form.scheduler.traditional.maxParallelTasks} 个任务；当前网络模式为${deviceModeLabel(form.networkLearning.deviceMode)}。`;
  }

  return `自动模式下总线程预算 ${form.scheduler.automatic.maxParallelThreads}，单任务上限 ${form.scheduler.automatic.maxThreadsPerTask}，当前策略为${profileLabel(form.scheduler.automatic.adaptiveProfile)}。`;
});

const proxySummary = computed(() => {
  if (form.proxy.mode === "disabled") {
    return "当前直接连接，不经过代理。";
  }

  if (form.proxy.mode === "system") {
    return "当前将跟随系统代理配置。";
  }

  return form.proxy.manualUrl.trim()
    ? `当前手动代理：${form.proxy.manualUrl.trim()}`
    : "请输入代理地址，例如 http://127.0.0.1:7890";
});

const downloadSummary = computed(() => {
  const location = form.download.defaultDownloadDir.trim() || "未设置默认路径";
  const checksumLabel =
    checksumOptions.find((option) => option.value === form.download.defaultChecksum)?.label ??
    form.download.defaultChecksum;

  return `默认位置：${location}；默认重试次数：${form.download.defaultMaxRetries}；全局校验方式：${checksumLabel}。`;
});

const networkLearningSummary = computed(() => {
  const scene = currentScene.value;
  if (!scene) {
    return "当前暂无网络学习画像。";
  }

  if (form.networkLearning.deviceMode === "mobile") {
    return `当前为${deviceModeLabel(form.networkLearning.deviceMode)}，不会累计网络画像；自动调度将回退到静态自适应策略。`;
  }

  if (!scene.learningEnabled) {
    return "网络学习已暂停，自动调度将回退到静态自适应策略。";
  }

  if (!scene.learnedMetrics) {
    return "暂无学习样本；后续自动模式下载会逐步建立网络画像。";
  }

  return `${deviceModeLabel(form.networkLearning.deviceMode)}；已累计 ${scene.learnedMetrics.sampleCount} 个样本，推荐初始线程 ${scene.learnedMetrics.recommendedInitialThreads}。`;
});

const networkMetricsCards = computed(() => {
  const metrics = currentScene.value?.learnedMetrics;
  const learningOpen =
    form.networkLearning.deviceMode !== "mobile" && Boolean(currentScene.value?.learningEnabled);

  return [
    {
      label: "学习状态",
      value: learningOpen ? "启用" : "停用",
    },
    {
      label: "估计带宽",
      value: metrics ? formatSpeed(metrics.estimatedBandwidthBps) : "—",
    },
    {
      label: "稳定性",
      value: metrics ? stabilityLabel(metrics.stabilityScore) : "—",
    },
    {
      label: "异常率",
      value: metrics ? formatPercent(metrics.penaltyRate) : "—",
    },
    {
      label: "推荐初始线程",
      value: metrics ? String(metrics.recommendedInitialThreads) : "—",
    },
    {
      label: "建议线程上限",
      value: metrics ? String(metrics.recommendedMaxThreadsPerTaskCap) : "—",
    },
    {
      label: "样本数",
      value: metrics ? String(metrics.sampleCount) : "0",
    },
    {
      label: "最近学习时间",
      value: metrics ? formatTimestamp(metrics.lastObservedAtMs) : "—",
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
  return adaptiveProfileOptions.find((option) => option.value === profile)?.label ?? profile;
}

function deviceModeLabel(mode: DeviceLearningMode) {
  return deviceModeOptions.find((option) => option.value === mode)?.label ?? mode;
}

function formatPercent(value: number) {
  return `${(value * 100).toFixed(value >= 0.1 ? 0 : 1)}%`;
}

function stabilityLabel(score: number) {
  if (score >= 0.88) {
    return "高";
  }
  if (score >= 0.68) {
    return "中";
  }
  return "低";
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

function copySingleNetworkScene(settings: NetworkLearningSettings): NetworkSceneProfile {
  const selectedScene =
    settings.scenes.find((scene) => scene.id === settings.currentSceneId) ?? settings.scenes[0];
  return {
    id: "default",
    name: "默认场景",
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
    showNotification(error instanceof Error ? error.message : "选择目录失败");
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
    showNotification("设置已保存");
  } catch (error) {
    showNotification(error instanceof Error ? error.message : "保存设置失败");
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
        <p class="section-kicker">Settings</p>
        <h2 class="panel-title">设置</h2>
      </div>
      <p class="settings-page__summary">{{ pageSummary }}</p>
    </div>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">Scheduler</p>
          <h3>线程分配</h3>
        </div>
        <span class="settings-section__icon i-ri-git-branch-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">分配模式</span>
          <UiSelect v-model="form.scheduler.mode" :options="schedulerModeOptions" />
        </label>

        <label v-if="form.scheduler.mode === 'traditional'" class="settings-field">
          <span class="settings-field__label">最大并行任务数</span>
          <UiNumberField v-model="form.scheduler.traditional.maxParallelTasks" :min="1" :max="32" />
          <p class="settings-field__hint">超过上限的任务会进入排队状态。</p>
        </label>

        <template v-else>
          <label class="settings-field">
            <span class="settings-field__label">最大并行线程数</span>
            <UiNumberField
              v-model="form.scheduler.automatic.maxParallelThreads"
              :min="1"
              :max="64"
            />
          </label>

          <label class="settings-field">
            <span class="settings-field__label">单任务线程数上限</span>
            <UiNumberField
              v-model="form.scheduler.automatic.maxThreadsPerTask"
              :min="1"
              :max="Math.max(1, form.scheduler.automatic.maxParallelThreads)"
            />
          </label>

          <label class="settings-field settings-field--wide">
            <span class="settings-field__label">自适应模式</span>
            <UiSelect
              v-model="form.scheduler.automatic.adaptiveProfile"
              :options="adaptiveProfileOptions"
            />
            <p class="settings-field__hint">
              保守更偏节制线程，平衡兼顾开销与速度，激进优先追求下载速度。
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
          {{ isSaving ? "保存中…" : "保存设置" }}
        </UiButton>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">Network Learning</p>
          <h3>网络环境学习</h3>
        </div>
        <span class="settings-section__icon i-ri-radar-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ networkLearningSummary }}</p>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">设备模式</span>
          <UiSelect v-model="form.networkLearning.deviceMode" :options="deviceModeOptions" />
          <p class="settings-field__hint">
            固定使用会积极学习，移动使用不学习，半移动使用会更保守地累计画像。
          </p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">是否允许学习</span>
          <span class="settings-toggle">
            <input
              v-if="currentScene"
              v-model="currentScene.learningEnabled"
              class="settings-toggle__control"
              type="checkbox"
            />
            <span class="settings-toggle__text">
              {{ currentScene?.learningEnabled ? "允许更新网络画像" : "暂停更新网络画像" }}
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
          {{ isSaving ? "保存中…" : "保存设置" }}
        </UiButton>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">Downloads</p>
          <h3>下载默认值</h3>
        </div>
        <span class="settings-section__icon i-ri-download-2-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ downloadSummary }}</p>

      <div class="settings-grid">
        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">默认下载位置</span>
          <div class="settings-directory-field">
            <UiInput
              v-model="form.download.defaultDownloadDir"
              type="text"
              placeholder="未设置时创建任务仍需手动选择"
            />
            <UiButton
              type="button"
              variant="secondary"
              size="sm"
              :loading="isPickingDirectory"
              @click="pickDefaultDownloadDirectory"
            >
              {{ isPickingDirectory ? "打开中…" : "浏览" }}
            </UiButton>
          </div>
          <p class="settings-field__hint">
            新建任务时会自动带入该目录，你仍然可以在任务里临时改掉。
          </p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">默认重试次数</span>
          <UiNumberField v-model="form.download.defaultMaxRetries" :min="0" :max="20" />
        </label>

        <label class="settings-field">
          <span class="settings-field__label">全局校验方式</span>
          <UiSelect v-model="form.download.defaultChecksum" :options="checksumOptions" />
          <p class="settings-field__hint">新建任务中不再单独显示校验方式，统一使用这里的设置。</p>
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
          {{ isSaving ? "保存中…" : "保存设置" }}
        </UiButton>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">Network</p>
          <h3>代理</h3>
        </div>
        <span class="settings-section__icon i-ri-global-line" aria-hidden="true" />
      </div>

      <p class="settings-section__summary">{{ proxySummary }}</p>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">代理模式</span>
          <UiSelect v-model="form.proxy.mode" :options="proxyModeOptions" />
        </label>

        <label v-if="form.proxy.mode === 'manual'" class="settings-field settings-field--wide">
          <span class="settings-field__label">代理地址</span>
          <UiInput v-model="form.proxy.manualUrl" type="text" placeholder="http://127.0.0.1:7890" />
          <p class="settings-field__hint">
            支持常见 HTTP / HTTPS / SOCKS 代理地址，按完整 URL 填写。
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
          {{ isSaving ? "保存中…" : "保存设置" }}
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
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
}

.settings-field {
  display: grid;
  gap: 0.45rem;
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
