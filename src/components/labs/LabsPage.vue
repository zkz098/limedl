<script setup lang="ts">
import { computed, ref, toRef } from "vue";

import { useI18n } from "../../i18n";
import { useNotification } from "../../composables/useNotification";
import { saveAppSettings } from "../../lib/tauri/settings-api";
import type {
  AdaptiveProfile,
  AppSettings,
  DeviceLearningMode,
  LogLevel,
} from "../../types/settings";
import type { ChecksumMode } from "../../types/download";
import UiButton from "../ui/UiButton.vue";

import LabsCdnAccelerationPanel from "./LabsCdnAccelerationPanel.vue";
import LabsNetworkLearningPanel from "./LabsNetworkLearningPanel.vue";

import {
  serializeSettings,
  useSettingsForm,
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

// ── Reactive form (shared composable) ─────────────────────────────

const { form, buildSettingsPayload, savedSettingsSnapshot } = useSettingsForm({
  settings: toRef(props, "settings"),
  t,
  onDirtyChange: (isDirty) => emit("dirtyChange", isDirty),
});

// ── State ────────────────────────────────────────────────────────

const isSaving = ref(false);

// ── Summaries ────────────────────────────────────────────────────

const optionArrays: SettingsOptionArrays = {
  adaptiveProfileOptions,
  deviceModeOptions,
  checksumOptions,
  logLevelOptions,
};

const { networkLearningSummary, networkMetricsCards } = useSettingsSummaries(form, t, optionArrays);

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
  background: var(--color-panel-muted);
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
  background: var(--color-panel);
}

.labs-tab--active {
  color: var(--color-heading);
  border-color: var(--color-border);
  background: var(--color-panel);
  font-weight: 600;
}

.labs-tab-content {
  display: grid;
  gap: 1rem;
}

.labs-save-bar {
  position: sticky;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 10;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--color-border);
  background: var(--color-panel);
  box-shadow: var(--shadow-card);
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
  border-radius: var(--radius-lg);
  background: var(--color-panel);
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
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
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
  background: var(--color-panel);
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
  background: var(--color-panel-muted);
}

.labs .settings-toggle:focus-visible,
.labs-page .settings-toggle:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.labs .settings-toggle--active,
.labs-page .settings-toggle--active {
  border-color: var(--color-accent-strong);
  background: var(--color-accent-soft);
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
  background: var(--color-panel-muted);
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
  font-family: var(--font-mono);
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