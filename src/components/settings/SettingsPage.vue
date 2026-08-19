<script setup lang="ts">
import { computed, ref, toRef, watch } from "vue";

import { useI18n } from "../../i18n";
import { useAsyncGuard } from "../../composables/useAsyncGuard";
import { useNotificationStore } from "../../stores/notification";
import { pickDirectory } from "../../lib/tauri/dialog-api";
import { fetchTrackerList, openLogDir, saveAppSettings } from "../../lib/tauri/settings-api";
import type { ChecksumMode } from "../../types/download";
import type { SupportedLanguage } from "../../i18n/resources";
import type {
  AdaptiveProfile,
  AppSettings,
  BackgroundOpacityPreset,
  ColorMode,
  LogLevel,
  ProxyMode,
  SchedulerMode,
} from "../../types/settings";
import UiButton from "../ui/UiButton.vue";

import SettingsAppearancePanel from "./SettingsAppearancePanel.vue";
import SettingsAria2RpcPanel from "./SettingsAria2RpcPanel.vue";
import SettingsBtPanel from "./SettingsBtPanel.vue";
import SettingsDownloadDefaultsPanel from "./SettingsDownloadDefaultsPanel.vue";
import SettingsIoBaselinePanel from "./SettingsIoBaselinePanel.vue";
import SettingsLoggingPanel from "./SettingsLoggingPanel.vue";
import SettingsProxyPanel from "./SettingsProxyPanel.vue";
import SettingsSchedulerPanel from "./SettingsSchedulerPanel.vue";
import SettingsAboutPanel from "./SettingsAboutPanel.vue";

import {
  DEFAULT_HTTP_USER_AGENT,
  DEFAULT_TRACKER_LIST_URL,
  serializeSettings,
  useSettingsForm,
  useSettingsSummaries,
  type SettingsOptionArrays,
} from "./settingsComposables";

const props = defineProps<{
  settings: AppSettings | null;
  gameMode?: boolean;
  bufferUsageBytes?: number;
  bufferLimitBytes?: number;
  activeSlots?: number;
  maxSlots?: number;
  queuedCount?: number;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
  dirtyChange: [isDirty: boolean];
  restartSetup: [];
}>();

const { language, languageOptions, setLanguage, t } = useI18n();
const { notifySuccess, notifyError, notifyInfo } = useNotificationStore();

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

// ── Reactive form (shared composable) ─────────────────────────────

const { form, buildSettingsPayload, savedSettingsSnapshot } = useSettingsForm({
  settings: toRef(props, "settings"),
  onDirtyChange: (isDirty) => emit("dirtyChange", isDirty),
});

// ── State──────────────────────────────────────────────────────────

const { isBusy: isSaving, run: runSave } = useAsyncGuard();
const { isBusy: isPickingDirectory, run: runPickDirectory } = useAsyncGuard();
const { isBusy: isPickingLogDirectory, run: runPickLogDirectory } = useAsyncGuard();
const { isBusy: isOpeningLogDir, run: runOpenLogDir } = useAsyncGuard();
const { isBusy: isFetchingTrackerList, run: runFetchTrackerList } = useAsyncGuard();

// ── Summaries (from composable)────────────────────────────────────

const optionArrays: SettingsOptionArrays = {
  adaptiveProfileOptions,
  checksumOptions,
  logLevelOptions,
};

const {
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

// ── Constraints: maxThreadsPerTask ≤ maxParallelThreads, minThreadsPerTask ≤ maxThreadsPerTask ──

watch(
  [
    () => form.scheduler.automatic.maxParallelThreads,
    () => form.scheduler.automatic.maxThreadsPerTask,
  ],
  ([maxParallel, maxPerTask]) => {
    if (form.scheduler.automatic.maxThreadsPerTask > maxParallel) {
      form.scheduler.automatic.maxThreadsPerTask = maxParallel;
    }
    if (form.scheduler.automatic.minThreadsPerTask > maxPerTask) {
      form.scheduler.automatic.minThreadsPerTask = maxPerTask;
    }
  },
);

// ── Helpers ───────────────────────────────────────────────────────

function changeLanguage(nextLanguage: SupportedLanguage) {
  void setLanguage(nextLanguage);
}

// ── Actions ───────────────────────────────────────────────────────

async function pickDefaultDownloadDirectory() {
  await runPickDirectory(async () => {
    try {
      const selectedPath = await pickDirectory();
      if (selectedPath) {
        form.download.defaultDownloadDir = selectedPath;
      }
    } catch (error) {
      notifyError(
        error instanceof Error ? error.message : t("settings.notifications.chooseDirectoryFailed"),
      );
    }
  });
}

async function pickLogDirectory() {
  await runPickLogDirectory(async () => {
    try {
      const selectedPath = await pickDirectory();
      if (selectedPath) {
        const cur = form.logging.filePath.trim();
        const sepIdx = Math.max(cur.lastIndexOf("/"), cur.lastIndexOf("\\"));
        const basename = sepIdx >= 0 ? cur.slice(sepIdx + 1) : cur;
        const filename = basename.includes(".") ? basename : "limedl.log";
        const sep = navigator.userAgent.includes("Windows") ? "\\" : "/";
        // Strip any trailing path separators. A manual loop avoids the regex
        // that Sonar S8786 flags as potentially involving non-linear
        // backtracking (path separators are single chars, so this is clearer).
        let trimmed = selectedPath;
        while (trimmed.length > 1 && (trimmed.endsWith("/") || trimmed.endsWith("\\"))) {
          trimmed = trimmed.slice(0, -1);
        }
        form.logging.filePath = trimmed + sep + filename;
      }
    } catch (error) {
      notifyError(
        error instanceof Error ? error.message : t("settings.notifications.chooseDirectoryFailed"),
      );
    }
  });
}

async function openLogDirectory() {
  await runOpenLogDir(async () => {
    try {
      await openLogDir();
    } catch (error) {
      notifyError(
        error instanceof Error ? error.message : t("settings.notifications.openLogDirFailed"),
      );
    }
  });
}

async function updateTrackerListFromUrl() {
  await runFetchTrackerList(async () => {
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
        error instanceof Error
          ? error.message
          : t("settings.notifications.trackerListUpdateFailed"),
      );
    }
  });
}

async function persistSettings() {
  return (
    (await runSave(async () => {
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
      }
    })) ?? false
  );
}

const activeTab = ref("appearance");
const isAdvancedExpanded = ref(false);

// Auto-expand advanced section when an advanced tab is selected
watch(activeTab, (tab) => {
  if (advancedTabs.some((at) => at.id === tab)) {
    isAdvancedExpanded.value = true;
  }
});

const commonTabs = [
  { id: "appearance", icon: "i-ri-palette-line", labelKey: "settings.appearanceKicker" },
  { id: "downloads", icon: "i-ri-download-line", labelKey: "settings.downloads" },
  { id: "proxy", icon: "i-ri-global-line", labelKey: "settings.proxyTitle" },
  { id: "about", icon: "i-ri-information-line", labelKey: "settings.aboutKicker" },
] as const;

const advancedTabs = [
  { id: "scheduler", icon: "i-ri-dashboard-line", labelKey: "settings.scheduler" },
  { id: "bt", icon: "i-ri-seedling-line", labelKey: "settings.bt" },
  { id: "io", icon: "i-ri-hard-drive-2-line", labelKey: "settings.io" },
  { id: "aria2Rpc", icon: "i-ri-terminal-box-line", labelKey: "settings.aria2Rpc" },
  { id: "logging", icon: "i-ri-file-list-3-line", labelKey: "settings.logging" },
] as const;

defineExpose({
  persistSettings,
});
</script>

<template>
  <section class="settings-page flex flex-col gap-4 flex-1 min-h-0 overflow-hidden">
    <div class="desk-panel__header settings-page__header flex-none items-end">
      <div>
        <p class="section-kicker">{{ t("settings.kicker") }}</p>
        <h2 class="panel-title">{{ t("settings.title") }}</h2>
      </div>
    </div>

    <div class="settings-page__layout flex flex-1 gap-5 min-h-0 overflow-hidden">
      <aside
        class="settings-page__sidebar w-52 flex-none flex flex-col gap-3 pb-4"
        role="tablist"
        :aria-label="t('settings.title')"
      >
        <nav class="settings-page__tabs flex flex-col gap-1">
          <!-- Common tabs: always visible -->
          <button
            v-for="tab in commonTabs"
            :key="tab.id"
            type="button"
            role="tab"
            :class="[
              'settings-page__tab',
              activeTab === tab.id ? 'settings-page__tab--active' : '',
            ]"
            :aria-selected="activeTab === tab.id"
            @click="activeTab = tab.id"
          >
            <span :class="tab.icon" aria-hidden="true" />
            <span>{{ t(tab.labelKey) }}</span>
          </button>

          <!-- Advanced divider + toggle -->
          <div class="settings-page__advanced-divider" />
          <button
            type="button"
            class="settings-page__advanced-toggle"
            :aria-expanded="isAdvancedExpanded"
            @click="isAdvancedExpanded = !isAdvancedExpanded"
          >
            <span
              class="settings-page__advanced-toggle-icon"
              :class="{ 'settings-page__advanced-toggle-icon--collapsed': !isAdvancedExpanded }"
            >
              <span class="i-ri-arrow-down-s-line" aria-hidden="true" />
            </span>
            <span>{{ t("settings.advancedSettings") }}</span>
          </button>

          <!-- Advanced tabs: collapsible -->
          <Transition name="advanced-tabs">
            <div v-show="isAdvancedExpanded" class="settings-page__advanced-tabs">
              <button
                v-for="tab in advancedTabs"
                :key="tab.id"
                type="button"
                role="tab"
                :class="[
                  'settings-page__tab',
                  activeTab === tab.id ? 'settings-page__tab--active' : '',
                ]"
                :aria-selected="activeTab === tab.id"
                @click="activeTab = tab.id"
              >
                <span :class="tab.icon" aria-hidden="true" />
                <span>{{ t(tab.labelKey) }}</span>
              </button>
            </div>
          </Transition>
        </nav>

        <div class="settings-page__save flex-none flex flex-col gap-2 mt-auto pt-3">
          <p class="settings-page__save-hint m-0 text-xs leading-[1.45]">
            {{ t("settings.saveHint") }}
          </p>
          <UiButton
            type="button"
            icon="i-ri-save-line"
            block
            :loading="isSaving"
            @click="persistSettings"
          >
            {{ isSaving ? t("common.saving") : t("common.save") }}
          </UiButton>
        </div>
      </aside>

      <div class="settings-page__content flex-1 overflow-y-auto min-w-0 min-h-0 pb-4">
        <SettingsAppearancePanel
          v-show="activeTab === 'appearance'"
          v-model:draft="form"
          :t="t"
          :language="language"
          :language-options="languageOptions"
          :color-mode-options="colorModeOptions"
          :background-opacity-options="backgroundOpacityOptions"
          @change-language="changeLanguage"
        />

        <SettingsSchedulerPanel
          v-show="activeTab === 'scheduler'"
          v-model:draft="form"
          :t="t"
          :scheduler-mode-options="schedulerModeOptions"
          :adaptive-profile-options="adaptiveProfileOptions"
          :global-speed-limit-mi-bps="globalSpeedLimitMiBps"
          @update:globalSpeedLimitMiBps="setGlobalSpeedLimitMiBps"
        />

        <SettingsDownloadDefaultsPanel
          v-show="activeTab === 'downloads'"
          v-model:draft="form"
          :t="t"
          :checksum-options="checksumOptions"
          :download-summary="downloadSummary"
          :is-picking-directory="isPickingDirectory"
          :default-user-agent-placeholder="DEFAULT_HTTP_USER_AGENT"
          @pick-directory="pickDefaultDownloadDirectory"
        />

        <SettingsBtPanel
          v-show="activeTab === 'bt'"
          v-model:draft="form"
          :t="t"
          :bt-summary="btSummary"
          :bt-upload-limit-mi-b="btUploadLimitMiB"
          :is-fetching-tracker-list="isFetchingTrackerList"
          :default-tracker-list-url="DEFAULT_TRACKER_LIST_URL"
          @update:btUploadLimitMiB="setBtUploadLimitMiB"
          @fetch-tracker-list="updateTrackerListFromUrl"
        />

        <SettingsIoBaselinePanel
          v-show="activeTab === 'io'"
          v-model:draft="form"
          :t="t"
          :game-mode="gameMode ?? false"
          :buffer-usage-bytes="bufferUsageBytes ?? 0"
          :buffer-limit-bytes="bufferLimitBytes ?? 0"
          :active-slots="activeSlots ?? 0"
          :max-slots="maxSlots ?? 0"
          :queued-count="queuedCount ?? 0"
        />

        <SettingsAria2RpcPanel v-show="activeTab === 'aria2Rpc'" v-model:draft="form" :t="t" />

        <SettingsLoggingPanel
          v-show="activeTab === 'logging'"
          v-model:draft="form"
          :t="t"
          :log-level-options="logLevelOptions"
          :logging-summary="loggingSummary"
          :is-picking-log-directory="isPickingLogDirectory"
          :is-opening-log-dir="isOpeningLogDir"
          @pick-log-directory="pickLogDirectory"
          @open-log-dir="openLogDirectory"
        />

        <SettingsProxyPanel
          v-show="activeTab === 'proxy'"
          v-model:draft="form"
          :t="t"
          :proxy-mode-options="proxyModeOptions"
          :proxy-summary="proxySummary"
        />

        <SettingsAboutPanel v-show="activeTab === 'about'" @restart-setup="emit('restartSetup')" />
      </div>
    </div>
  </section>
</template>

<style>
/* ── LabsPage sidebar layout (SettingsPage migrated to utilities) ── */

.settings-page__layout,
.labs-page__layout {
  flex: 1 1 auto;
  display: flex;
  gap: var(--space-5);
  min-height: 0;
  overflow: hidden;
}

.settings-page__sidebar,
.labs-page__sidebar {
  width: 13rem;
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-bottom: var(--space-4);
}

.settings-page__tabs,
.labs-page__tabs {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.settings-page__tab,
.labs-page__tab {
  position: relative;
  min-height: 2.75rem;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0 0.9rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 0.9rem;
  text-align: left;
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    color 0.2s ease;
}

.settings-page__tab:hover,
.labs-page__tab:hover {
  color: var(--color-heading);
  background: var(--color-surface-muted);
}

.settings-page__tab:focus-visible,
.labs-page__tab:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.settings-page__tab--active,
.labs-page__tab--active {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
  font-weight: var(--font-weight-semibold);
}

.settings-page__tab--active::before,
.labs-page__tab--active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0.55rem;
  bottom: 0.55rem;
  width: 3px;
  border-radius: 0 2px 2px 0;
  background: var(--color-accent-strong);
}

.settings-page__tab > [class*="i-ri-"],
.labs-page__tab > [class*="i-ri-"] {
  flex: 0 0 auto;
  font-size: 1.05rem;
}

/* Advanced settings section */
.settings-page__advanced-divider {
  border-top: 1px solid var(--color-border);
  margin: var(--space-2) 0;
}

.settings-page__advanced-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-height: 2.75rem;
  padding: 0 0.9rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 0.85rem;
  font-weight: 600;
  text-align: left;
  transition:
    background-color 0.2s ease,
    color 0.2s ease;
}

.settings-page__advanced-toggle:hover {
  color: var(--color-heading);
  background: var(--color-surface-muted);
}

.settings-page__advanced-toggle:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.settings-page__advanced-toggle-icon {
  flex: 0 0 auto;
  font-size: 1rem;
  transition: transform 0.2s ease;
  display: inline-flex;
  align-items: center;
}

.settings-page__advanced-toggle-icon--collapsed {
  transform: rotate(-90deg);
}

.settings-page__advanced-tabs {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.advanced-tabs-enter-active,
.advanced-tabs-leave-active {
  transition:
    max-height 0.25s ease,
    opacity 0.2s ease;
  overflow: hidden;
}

.advanced-tabs-enter-from,
.advanced-tabs-leave-to {
  max-height: 0;
  opacity: 0;
}

.advanced-tabs-enter-to,
.advanced-tabs-leave-from {
  max-height: 500px;
  opacity: 1;
}

.settings-page__save,
.labs-page__save {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-top: auto;
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border);
}

.settings-page__save-hint,
.labs-page__save-hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.8rem;
  line-height: 1.45;
}

.settings-page__content,
.labs-page__content {
  flex: 1 1 0;
  overflow-y: auto;
  min-width: 0;
  min-height: 0;
  padding-bottom: var(--space-4);
}

@media (max-width: 840px) {
  .settings-page__layout,
  .labs-page__layout {
    flex-direction: column;
    overflow: visible;
  }

  .settings-page__sidebar,
  .labs-page__sidebar {
    width: auto;
    flex-direction: row;
    align-items: center;
    overflow-x: auto;
    gap: var(--space-2);
    padding-bottom: 0;
  }

  .settings-page__content,
  .labs-page__content {
    overflow-y: visible;
  }

  .settings-page__tabs,
  .labs-page__tabs {
    flex-direction: row;
    flex: 1 1 auto;
    flex-wrap: wrap;
  }

  .settings-page__tab,
  .labs-page__tab {
    flex: 0 0 auto;
    min-height: 2.25rem;
    padding: 0 0.75rem;
    white-space: nowrap;
  }

  .settings-page__save,
  .labs-page__save {
    flex-direction: row;
    align-items: center;
    padding-top: 0;
    padding-left: var(--space-3);
    border-top: none;
    border-left: 1px solid var(--color-border);
  }

  .settings-page__save-hint,
  .labs-page__save-hint {
    display: none;
  }

  .settings-page__advanced-divider {
    border-top: none;
    border-left: 1px solid var(--color-border);
    margin: 0 0.5rem;
    height: 1.5rem;
  }

  .settings-page__advanced-toggle {
    min-height: 2.25rem;
    padding: 0 0.75rem;
    white-space: nowrap;
  }

  .settings-page__advanced-tabs {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .advanced-tabs-enter-active,
  .advanced-tabs-leave-active {
    transition:
      max-width 0.25s ease,
      opacity 0.2s ease;
    overflow: hidden;
  }

  .advanced-tabs-enter-from,
  .advanced-tabs-leave-to {
    max-width: 0;
    opacity: 0;
  }

  .advanced-tabs-enter-to,
  .advanced-tabs-leave-from {
    max-width: 500px;
    opacity: 1;
  }
}

/* ── Shared structural classes for settings & labs panels ────────── */
/* NON-SCOPED: the settings/labs child panel components require these  */
/* classes to render correctly. This block is intentionally shared     */
/* across SettingsPage and LabsPage to avoid duplication.              */
/* ────────────────────────────────────────────────────────────────── */

.settings-page .settings-section {
  display: grid;
  gap: 1rem;
}

.settings-page .settings-grid,
.labs-page .settings-grid {
  display: grid;
  align-items: start;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
}

.settings-page .settings-field,
.labs-page .settings-field {
  display: grid;
  gap: 0.45rem;
  align-content: start;
  grid-auto-rows: max-content;
  min-width: 0;
}

.settings-page .settings-field--wide,
.labs-page .settings-field--wide {
  grid-column: 1 / -1;
}

.settings-page .settings-field__label,
.labs-page .settings-field__label {
  color: var(--color-heading);
  font-size: 0.9rem;
  font-weight: 600;
  letter-spacing: 0;
  text-transform: none;
}

.settings-page .settings-field__hint,
.labs-page .settings-field__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  line-height: 1.5;
}

.settings-page .settings-directory-field,
.labs-page .settings-directory-field,
.settings-page .settings-inline-field,
.labs-page .settings-inline-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.75rem;
}

/* Textarea used in settings panels */

.settings-page .settings-textarea,
.labs-page .settings-textarea {
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

.settings-page .settings-textarea::placeholder,
.labs-page .settings-textarea::placeholder {
  color: var(--color-text-soft);
}

.settings-page .settings-textarea:hover:not(:focus-visible),
.labs-page .settings-textarea:hover:not(:focus-visible) {
  border-color: var(--color-border-strong);
}

.settings-page .settings-textarea:focus-visible,
.labs-page .settings-textarea:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.settings-page .settings-metrics-grid,
.labs-page .settings-metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.85rem;
}

.settings-page .settings-metric-card,
.labs-page .settings-metric-card {
  display: grid;
  gap: 0.35rem;
  padding: 0.85rem 0.9rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
}

.settings-page .settings-metric-card__label,
.labs-page .settings-metric-card__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.settings-page .settings-metric-card__value,
.labs-page .settings-metric-card__value {
  color: var(--color-heading);
  font-family: var(--font-mono);
  font-size: 0.95rem;
  line-height: 1.4;
}

@media (max-width: 960px) {
  .settings-page .settings-grid,
  .labs-page .settings-grid,
  .settings-page .settings-metrics-grid,
  .labs-page .settings-metrics-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 840px) {
  .settings-page .settings-grid,
  .labs-page .settings-grid,
  .settings-page .settings-metrics-grid,
  .labs-page .settings-metrics-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-page .settings-field--wide,
  .labs-page .settings-field--wide {
    grid-column: auto;
  }

  .settings-page .settings-directory-field,
  .labs-page .settings-directory-field,
  .settings-page .settings-inline-field,
  .labs-page .settings-inline-field {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
