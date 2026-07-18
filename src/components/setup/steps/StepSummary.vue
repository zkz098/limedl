<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings } from "../../../types/settings";
import UiBadge from "../../ui/UiBadge.vue";
import UiButton from "../../ui/UiButton.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
  "edit-step": [index: number];
}>();

const { t, language } = useI18n();

const editStepIndex = {
  language: 1,
  cdn: 2,
  rpc: 3,
  performance: 4,
  directory: 5,
  appearance: 6,
  autostart: 7,
} as const;

const summaryRows = [
  {
    key: "language",
    icon: "i-ri-translate-2",
    label: "summaryLanguage",
    value: getLanguageLabel,
  },
  {
    key: "cdn",
    icon: "i-ri-flashlight-fill",
    label: "summaryCdn",
    value: () =>
      props.settings.cdnAcceleration.enabled ? t("common.enabled") : t("common.disabled"),
    tone: () =>
      (props.settings.cdnAcceleration.enabled ? "success" : "neutral") as "success" | "neutral",
    isBadge: true,
  },
  {
    key: "rpc",
    icon: "i-ri-server-line",
    label: "summaryRpc",
    value: getRpcLabel,
  },
  {
    key: "directory",
    icon: "i-ri-folder-download-line",
    label: "summaryDirectory",
    value: getDirectoryLabel,
    isPath: true,
  },
  {
    key: "appearance",
    icon: "i-ri-palette-line",
    label: "summaryAppearance",
    value: getAppearanceLabel,
  },
  {
    key: "performance",
    icon: "i-ri-speed-up-line",
    label: "stepPerformance",
    value: getPerformanceLabel,
  },
  {
    key: "autostart",
    icon: "i-ri-settings-3-line",
    label: "stepAutostart",
    value: getSystemLabel,
  },
];

function goToStep(index: number) {
  emit("edit-step", index);
}

function getLanguageLabel() {
  const lang = language.value ?? "zh-CN";
  return t(`language.${lang === "zh-CN" ? "zhCN" : "enUS"}`);
}

function getRpcLabel() {
  if (!props.settings.aria2Rpc.enabled) {
    return t("setupWizard.summaryRpcDisabled");
  }
  return t("setupWizard.summaryRpcEnabled", { port: props.settings.aria2Rpc.port });
}

function getDirectoryLabel() {
  const path = props.settings.download.defaultDownloadDir;
  return path ? path : t("setupWizard.summaryDirectoryEmpty");
}

function getAppearanceLabel() {
  const mode = t(`settings.colorModeNames.${props.settings.appearance.colorMode}`);
  const theme = t(`settings.themeColorNames.${props.settings.appearance.themeColor}`);
  const background = t(
    `settings.backgroundOpacityNames.${props.settings.appearance.backgroundOpacity}`,
  );
  return `${mode} · ${theme} · ${background}`;
}

function getPerformanceLabel() {
  const mode = props.settings.scheduler.mode === "automatic"
    ? t("tokens.automatic")
    : t("tokens.traditional");
  const chunk = props.settings.scheduler.chunkSizeStrategy === "adaptive"
    ? t("common.enabled")
    : t("common.disabled");
  return `${mode} · Chunk: ${chunk}`;
}

function getSystemLabel() {
  const autostart = props.settings.autostart ? t("common.enabled") : t("common.disabled");
  const notify = props.settings.notifications.enabled ? t("common.enabled") : t("common.disabled");
  const proxyLabel = t(`tokens.${props.settings.proxy.mode}`);
  return `${t("settings.autoStart")}: ${autostart} · ${t("settings.notificationSettings.title")}: ${notify} · ${proxyLabel}`;
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-file-list-3-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.summaryTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.summaryDescription") }}</p>
    <div class="setup-step__body">
      <div class="summary-card">
        <div
          v-for="row in summaryRows"
          :key="row.key"
          class="summary-row"
        >
          <span class="summary-row__icon" :class="row.icon" aria-hidden="true" />
          <div class="summary-row__info">
            <span class="summary-row__label">{{ t(`setupWizard.${row.label}`) }}</span>
            <UiBadge v-if="row.isBadge" :tone="row.tone()">
              {{ row.value() }}
            </UiBadge>
            <span
              v-else
              class="summary-row__value"
              :class="{ 'summary-row__value--path': row.isPath }"
              >{{ row.value() }}</span
            >
          </div>
          <UiButton
            class="summary-row__edit"
            variant="ghost"
            size="sm"
            icon="i-ri-edit-line"
            @click="goToStep(editStepIndex[row.key as keyof typeof editStepIndex])"
          >
            {{ t("setupWizard.summaryEdit") }}
          </UiButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup-step {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-6);
  flex: 1;
  min-height: 0;
  align-items: center;
  text-align: center;
}

.setup-step__header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}

.setup-step__title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.setup-step__description {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
  max-width: 480px;
}

.setup-step__body {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 560px;
}

.setup-step__icon {
  font-size: 2.5rem;
  color: var(--color-accent);
}

.summary-card {
  display: flex;
  flex-direction: column;
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
  overflow: hidden;
}

.summary-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--border-width-thin) solid var(--color-border);
  text-align: left;
  transition: background-color 0.2s ease;
}

.summary-row:last-child {
  border-bottom: none;
}

.summary-row:hover {
  background: var(--color-surface-muted);
}

.summary-row__icon {
  flex-shrink: 0;
  font-size: var(--font-size-metric);
  color: var(--color-accent);
}

.summary-row__info {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  flex: 1;
}

.summary-row__label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.summary-row__value {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-row__value--path {
  font-family: var(--font-mono);
  font-weight: 400;
  font-size: var(--font-size-small);
}

.summary-row__edit {
  transition: color 0.2s ease;
}

.summary-row__edit:hover {
  color: var(--color-accent-strong);
  text-decoration: underline;
  text-underline-offset: 0.15em;
}
</style>
