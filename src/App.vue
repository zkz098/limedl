<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from "vue";

import DownloadComposer from "./components/downloader/DownloadComposer.vue";
import DownloadInspector from "./components/downloader/DownloadInspector.vue";
import DownloadQueueTable from "./components/downloader/DownloadQueueTable.vue";
import SidebarBtStatus from "./components/sidebar/SidebarBtStatus.vue";
import LabsPage from "./components/labs/LabsPage.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import UiButton from "./components/ui/UiButton.vue";
import UiDialog from "./components/ui/UiDialog.vue";
import { formatSpeed } from "./lib/download-format";
import { getAppSettings } from "./lib/tauri/settings-api";
import { useDownloader } from "./composables/useDownloader";
import { useNotification } from "./composables/useNotification";
import { useI18n } from "./i18n";
import NotificationToast from "./components/ui/NotificationToast.vue";
import type { AppSettings, ColorMode } from "./types/settings";

const {
  actionName,
  canCancel,
  canPause,
  canResume,
  canPauseDownload,
  canResumeDownload,
  btRuntimeStatus,
  downloads,
  form,
  isAutoRefreshing,
  isPickingDirectory,
  isPickingMetalink,
  isPickingTorrent,
  isRefreshingList,
  isRefreshingStatus,
  isStarting,
  applyAppSettingsDefaults,
  pickDestinationDirectory,
  pickMetalinkSourceFile,
  pickTorrentSourceFile,
  refreshList,
  refreshStatus,
  runCancel,
  runCopyLink,
  runDeleteTask,
  runDeleteTaskPermanently,
  runOpenInExplorer,
  runPause,
  runPauseFor,
  runResume,
  runResumeFor,
  selectDownload,
  selectedId,
  selectedSnapshot,
  selectedSummary,
  submitStart,
} = useDownloader();

const { t } = useI18n();
const showComposerDialog = ref(false);
const inspectorCollapsed = ref(false);
const currentView = ref<"home" | "settings" | "labs">("home");
const appSettings = ref<AppSettings | null>(null);
const pendingPermanentDeleteId = ref<string | null>(null);
const pendingView = ref<"home" | "settings" | "labs" | null>(null);
const settingsHasUnsavedChanges = ref(false);
const labsHasUnsavedChanges = ref(false);
const isSavingBeforeNavigation = ref(false);
const settingsPageRef = useTemplateRef<InstanceType<typeof SettingsPage>>("settingsPage");
const labsPageRef = useTemplateRef<InstanceType<typeof LabsPage>>("labsPage");
const knownFailedDownloadIds = new Set<string>();
let hasSeenInitialDownloadList = false;
let colorSchemeQuery: MediaQueryList | null = null;

const { notifications, notifyError, dismiss } = useNotification();

function handleSystemColorSchemeChange() {
  applyColorMode(appSettings.value?.appearance?.colorMode ?? "system");
}

const selectedOverview = computed(() => selectedSnapshot.value ?? selectedSummary.value);
const showDetailInfo = computed(() => appSettings.value?.appearance?.showDetailInfo ?? true);
const showHeatmap = computed(() => appSettings.value?.appearance?.showHeatmap ?? true);
const showUnsavedSettingsDialog = computed(() => pendingView.value !== null);
const pendingViewIsLeavingLabs = computed(
  () => currentView.value === "labs" && pendingView.value !== null,
);
const unsavedDialogKicker = computed(() =>
  pendingViewIsLeavingLabs.value ? t("labs.kicker") : t("settings.kicker"),
);
const unsavedDialogTitle = computed(() =>
  pendingViewIsLeavingLabs.value ? t("labs.unsavedTitle") : t("dialog.unsavedSettingsTitle"),
);
const unsavedDialogMessage = computed(() =>
  pendingViewIsLeavingLabs.value ? t("labs.unsavedMessage") : t("dialog.unsavedSettingsMessage"),
);
const pendingPermanentDeleteTask = computed(
  () => downloads.value.find((download) => download.id === pendingPermanentDeleteId.value) ?? null,
);
const selectedStateLabel = computed(() =>
  selectedOverview.value?.state ? t(`states.${selectedOverview.value.state}`) : t("common.unknown"),
);
const activeSpeedLabel = computed(() => formatSpeed(selectedOverview.value?.speedBytesPerSecond));
const activeCount = computed(
  () =>
    downloads.value.filter((download) =>
      ["downloading", "retrying", "verifying"].includes(download.state),
    ).length,
);
const completedCount = computed(
  () => downloads.value.filter((download) => download.state === "completed").length,
);

const handleSubmitStart = async () => {
  await submitStart();
  showComposerDialog.value = false;
};

const handleRefreshSelected = async () => {
  if (!selectedId.value) {
    return;
  }

  await refreshStatus(selectedId.value);
};

const handleTaskPauseOrResume = async (downloadId: string) => {
  const target = downloads.value.find((download) => download.id === downloadId);

  if (!target) {
    return;
  }

  if (canPauseDownload(target)) {
    await runPauseFor(downloadId);
    return;
  }

  if (canResumeDownload(target)) {
    await runResumeFor(downloadId);
  }
};

function requestPermanentDelete(downloadId: string) {
  pendingPermanentDeleteId.value = downloadId;
}

function navigateTo(view: "home" | "settings" | "labs") {
  if (view === currentView.value) {
    return;
  }

  const leavingDirtyView =
    (currentView.value === "settings" && settingsHasUnsavedChanges.value) ||
    (currentView.value === "labs" && labsHasUnsavedChanges.value);

  if (leavingDirtyView) {
    pendingView.value = view;
    return;
  }

  currentView.value = view;
}

function cancelDiscardSettings() {
  pendingView.value = null;
}

function confirmDiscardSettings() {
  const nextView = pendingView.value;
  pendingView.value = null;
  settingsHasUnsavedChanges.value = false;
  labsHasUnsavedChanges.value = false;

  if (nextView) {
    currentView.value = nextView;
  }
}

async function saveSettingsAndNavigate() {
  if (isSavingBeforeNavigation.value) {
    return;
  }

  isSavingBeforeNavigation.value = true;
  try {
    let saved = false;
    if (currentView.value === "settings") {
      saved = (await settingsPageRef.value?.persistSettings()) ?? false;
    } else if (currentView.value === "labs") {
      saved = (await labsPageRef.value?.persistSettings()) ?? false;
    }

    if (!saved) {
      return;
    }

    const nextView = pendingView.value;
    pendingView.value = null;
    settingsHasUnsavedChanges.value = false;
    labsHasUnsavedChanges.value = false;
    if (nextView) {
      currentView.value = nextView;
    }
  } finally {
    isSavingBeforeNavigation.value = false;
  }
}

function cancelPermanentDelete() {
  if (actionName.value === "Purge") {
    return;
  }

  pendingPermanentDeleteId.value = null;
}

async function confirmPermanentDelete() {
  const downloadId = pendingPermanentDeleteId.value;
  if (!downloadId) {
    return;
  }

  await runDeleteTaskPermanently(downloadId);
  pendingPermanentDeleteId.value = null;
}

function handleSettingsSaved(nextSettings: AppSettings) {
  appSettings.value = nextSettings;
  settingsHasUnsavedChanges.value = false;
  applyAppearanceSettings(nextSettings);
  applyAppSettingsDefaults(nextSettings);
}

function handleSettingsDirtyChange(isDirty: boolean) {
  settingsHasUnsavedChanges.value = isDirty;
}

function handleLabsSaved(nextSettings: AppSettings) {
  appSettings.value = nextSettings;
  labsHasUnsavedChanges.value = false;
  applyAppearanceSettings(nextSettings);
  applyAppSettingsDefaults(nextSettings);
}

function handleLabsDirtyChange(isDirty: boolean) {
  labsHasUnsavedChanges.value = isDirty;
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

function applyAppearanceSettings(settings: AppSettings) {
  document.documentElement.dataset.theme = settings.appearance?.themeColor ?? "default";
  document.documentElement.dataset.surface = settings.appearance?.backgroundOpacity ?? "default";
  applyColorMode(settings.appearance?.colorMode ?? "system");
}

function resolveColorMode(mode: ColorMode) {
  if (mode !== "system") {
    return mode;
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyColorMode(mode: ColorMode) {
  document.documentElement.dataset.colorModePreference = mode;
  document.documentElement.dataset.colorMode = resolveColorMode(mode);
}

onMounted(() => {
  colorSchemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
  colorSchemeQuery.addEventListener("change", handleSystemColorSchemeChange);
  applyColorMode("system");
  void loadSettings();
});

watch(
  () => selectedId.value,
  (nextId) => {
    if (!nextId) {
      inspectorCollapsed.value = false;
    }
  },
);

watch(
  () => showComposerDialog.value,
  (isOpen) => {
    if (!isOpen || !appSettings.value) {
      return;
    }

    applyAppSettingsDefaults(appSettings.value);
  },
);

watch(
  downloads,
  (nextDownloads) => {
    if (!hasSeenInitialDownloadList) {
      for (const download of nextDownloads) {
        if (download.state === "failed") {
          knownFailedDownloadIds.add(download.id);
        }
      }
      hasSeenInitialDownloadList = true;
      return;
    }

    for (const download of nextDownloads) {
      if (download.state !== "failed") {
        knownFailedDownloadIds.delete(download.id);
        continue;
      }

      if (knownFailedDownloadIds.has(download.id)) {
        continue;
      }

      knownFailedDownloadIds.add(download.id);
      notifyError(
        t("messages.downloadFailed", {
          fileName: download.fileName,
          reason: download.error || t("common.unknown"),
        }),
      );
    }
  },
  { deep: true },
);

onBeforeUnmount(() => {
  colorSchemeQuery?.removeEventListener("change", handleSystemColorSchemeChange);
});
</script>

<template>
  <main class="app-shell min-h-screen text-[var(--color-text-main)]">
    <NotificationToast
      :notifications="notifications"
      @dismiss="dismiss"
    />

    <aside class="sidebar">
      <div class="sidebar__brand">
        <div class="sidebar__logo-mark" aria-hidden="true">
          <span class="i-ri-download-cloud-2-line" />
        </div>
        <div>
          <p class="section-kicker">Transfer Desk</p>
          <h1>Downloader</h1>
        </div>
      </div>

      <UiButton icon="i-ri-add-line" block @click="showComposerDialog = true">
        {{ t("nav.newTask") }}
      </UiButton>

      <nav class="sidebar-nav" :aria-label="t('nav.primary')">
        <button
          type="button"
          class="sidebar-nav__item"
          :class="{ 'sidebar-nav__item--active': currentView === 'home' }"
          @click="navigateTo('home')"
        >
          <span class="sidebar-nav__icon i-ri-home-5-line" aria-hidden="true" />
          <span>{{ t("nav.home") }}</span>
        </button>
        <button
          type="button"
          class="sidebar-nav__item"
          :class="{ 'sidebar-nav__item--active': currentView === 'settings' }"
          @click="navigateTo('settings')"
        >
          <span class="sidebar-nav__icon i-ri-settings-3-line" aria-hidden="true" />
          <span>{{ t("nav.settings") }}</span>
        </button>
        <button
          type="button"
          class="sidebar-nav__item"
          :class="{ 'sidebar-nav__item--active': currentView === 'labs' }"
          @click="navigateTo('labs')"
        >
          <span class="sidebar-nav__icon i-ri-flask-line" aria-hidden="true" />
          <span>{{ t("nav.labs") }}</span>
        </button>
      </nav>

      <div class="sidebar__divider" aria-hidden="true" />

      <div class="sidebar-overview">
        <p class="section-kicker">{{ t("sidebar.overview") }}</p>
        <div class="sidebar-overview__list">
          <p>
            <span>{{ t("sidebar.totalTasks") }}</span
            ><strong>{{ downloads.length }}</strong>
          </p>
          <p>
            <span>{{ t("sidebar.active") }}</span
            ><strong>{{ activeCount }}</strong>
          </p>
          <p>
            <span>{{ t("sidebar.completed") }}</span
            ><strong>{{ completedCount }}</strong>
          </p>
          <p>
            <span>{{ t("sidebar.currentSpeed") }}</span
            ><strong>{{ activeSpeedLabel }}</strong>
          </p>
          <p>
            <span>{{ t("sidebar.selectedState") }}</span>
            <strong>{{ selectedStateLabel }}</strong>
          </p>
        </div>
      </div>

      <SidebarBtStatus :status="btRuntimeStatus" />
    </aside>

    <section class="main-content">
      <DownloadQueueTable
        v-if="currentView === 'home'"
        :downloads="downloads"
        :is-auto-refreshing="isAutoRefreshing"
        :is-refreshing-list="isRefreshingList"
        :selected-id="selectedId"
        :task-action-name="actionName"
        @copy-link="runCopyLink"
        @delete-task="runDeleteTask"
        @delete-task-permanently="requestPermanentDelete"
        @open-in-explorer="runOpenInExplorer"
        @pause-or-resume="handleTaskPauseOrResume"
        @refresh="refreshList"
        @select="selectDownload"
      />
      <SettingsPage
        v-else-if="currentView === 'settings'"
        ref="settingsPage"
        :settings="appSettings"
        @dirty-change="handleSettingsDirtyChange"
        @saved="handleSettingsSaved"
      />
      <LabsPage
        v-else
        ref="labsPage"
        :settings="appSettings"
        @dirty-change="handleLabsDirtyChange"
        @saved="handleLabsSaved"
      />
    </section>

    <Transition name="floating-inspector">
      <div
        v-if="currentView === 'home' && selectedOverview"
        class="floating-inspector"
        :class="{ 'is-collapsed': inspectorCollapsed }"
      >
        <button
          type="button"
          class="floating-inspector__tab"
          @click="inspectorCollapsed = !inspectorCollapsed"
        >
          <div class="floating-inspector__tab-copy">
            <span class="floating-inspector__tab-kicker">{{ t("inspector.taskDetails") }}</span>
            <strong>{{ selectedOverview.fileName }}</strong>
          </div>
          <div class="floating-inspector__tab-meta">
            <span>{{ selectedStateLabel }}</span>
            <span
              class="floating-inspector__tab-icon"
              :class="inspectorCollapsed ? 'i-ri-arrow-up-s-line' : 'i-ri-arrow-down-s-line'"
              aria-hidden="true"
            />
          </div>
        </button>

        <div v-show="!inspectorCollapsed" class="floating-inspector__body">
          <DownloadInspector
            :action-name="actionName"
            :can-cancel="canCancel"
            :can-pause="canPause"
            :can-resume="canResume"
            :is-refreshing-status="isRefreshingStatus"
            :selected-overview="selectedOverview"
            :selected-snapshot="selectedSnapshot"
            :show-detail-info="showDetailInfo"
            :show-heatmap="showHeatmap"
            @cancel="runCancel"
            @pause="runPause"
            @refresh="handleRefreshSelected"
            @resume="runResume"
            @close="selectDownload(null)"
          />
        </div>
      </div>
    </Transition>

    <UiDialog v-model="showComposerDialog" width="min(46rem, calc(100vw - 1.5rem))">
      <template #title>
        <div class="dialog-heading">
          <div>
            <p class="section-kicker">{{ t("dialog.newTransfer") }}</p>
            <h2>{{ t("dialog.newTaskTitle") }}</h2>
          </div>
          <span class="dialog-heading__icon i-ri-download-cloud-2-line" aria-hidden="true" />
        </div>
      </template>

      <DownloadComposer
        :form="form"
        :is-picking-directory="isPickingDirectory"
        :is-picking-metalink="isPickingMetalink"
        :is-picking-torrent="isPickingTorrent"
        :is-starting="isStarting"
        :settings="appSettings"
        @pick-directory="pickDestinationDirectory"
        @pick-metalink="pickMetalinkSourceFile"
        @pick-torrent="pickTorrentSourceFile"
        @submit="handleSubmitStart"
      />
    </UiDialog>

    <UiDialog
      :model-value="Boolean(pendingPermanentDeleteId)"
      width="min(32rem, calc(100vw - 1.5rem))"
      :close-on-overlay="actionName !== 'Purge'"
      @update:model-value="
        (value) => {
          if (!value) cancelPermanentDelete();
        }
      "
    >
      <template #title>
        <div class="dialog-heading">
          <div>
            <p class="section-kicker">{{ t("dialog.confirmDelete") }}</p>
            <h2>{{ t("dialog.permanentDeleteTitle") }}</h2>
          </div>
          <span
            class="dialog-heading__icon dialog-heading__icon--danger i-ri-delete-bin-line"
            aria-hidden="true"
          />
        </div>
      </template>

      <div class="confirm-delete">
        <p class="confirm-delete__message">
          {{ t("dialog.permanentDeleteMessage") }}
        </p>
        <div v-if="pendingPermanentDeleteTask" class="confirm-delete__target">
          <span>{{ t("dialog.targetFile") }}</span>
          <strong>{{ pendingPermanentDeleteTask.fileName }}</strong>
        </div>
        <div class="confirm-delete__actions">
          <UiButton
            type="button"
            variant="secondary"
            :disabled="actionName === 'Purge'"
            @click="cancelPermanentDelete"
          >
            {{ t("common.cancel") }}
          </UiButton>
          <UiButton
            type="button"
            variant="danger"
            icon="i-ri-delete-bin-line"
            :loading="actionName === 'Purge'"
            @click="confirmPermanentDelete"
          >
            {{ actionName === "Purge" ? t("dialog.deleting") : t("dialog.confirmPermanentDelete") }}
          </UiButton>
        </div>
      </div>
    </UiDialog>

    <UiDialog
      :model-value="showUnsavedSettingsDialog"
      width="min(32rem, calc(100vw - 1.5rem))"
      @update:model-value="
        (value) => {
          if (!value) cancelDiscardSettings();
        }
      "
    >
      <template #title>
        <div class="dialog-heading">
          <div>
            <p class="section-kicker">{{ unsavedDialogKicker }}</p>
            <h2>{{ unsavedDialogTitle }}</h2>
          </div>
          <span class="dialog-heading__icon i-ri-error-warning-line" aria-hidden="true" />
        </div>
      </template>

      <div class="confirm-delete">
        <p class="confirm-delete__message">
          {{ unsavedDialogMessage }}
        </p>
        <div class="confirm-delete__actions">
          <UiButton type="button" variant="secondary" @click="cancelDiscardSettings">
            {{ t("dialog.keepEditing") }}
          </UiButton>
          <UiButton
            type="button"
            icon="i-ri-save-line"
            :loading="isSavingBeforeNavigation"
            @click="saveSettingsAndNavigate"
          >
            {{ isSavingBeforeNavigation ? t("common.saving") : t("dialog.saveSettingsAndLeave") }}
          </UiButton>
          <UiButton
            type="button"
            variant="danger"
            icon="i-ri-arrow-right-line"
            @click="confirmDiscardSettings"
          >
            {{ t("dialog.discardSettings") }}
          </UiButton>
        </div>
      </div>
    </UiDialog>
  </main>
</template>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: minmax(14.5rem, 16rem) minmax(0, 1fr);
  height: 100vh;
  overflow: hidden;
  background: var(--color-bg-base);
}

.sidebar,
.main-content {
  position: relative;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  height: 100vh;
  padding: 1rem;
  border-right: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  overflow: hidden;
}

.sidebar__brand h1 {
  margin: 0.15rem 0 0;
  font-family: var(--font-display);
  font-size: 1.5rem;
  font-weight: 600;
  line-height: 1;
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.sidebar__brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.sidebar__logo-mark {
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  font-size: 1.15rem;
}

.panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.875rem;
}

.panel-head__icon,
.dialog-heading__icon {
  font-size: 1.25rem;
  color: var(--color-text-muted);
}

.dialog-heading__icon--danger {
  color: var(--color-danger-text);
}

.sidebar-nav {
  display: grid;
  gap: 0.15rem;
}

.sidebar-nav__item {
  width: 100%;
  min-height: 2.25rem;
  display: flex;
  align-items: center;
  gap: 0.625rem;
  padding: 0 0.625rem;
  border: 0;
  border-left: 2px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  text-align: left;
  font-size: var(--font-size-small);
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.sidebar-nav__item:hover {
  background: var(--color-surface-muted);
  color: var(--color-text-main);
}

.sidebar-nav__item--active {
  background: var(--color-surface-muted);
  border-left-color: var(--color-accent);
  color: var(--color-accent-strong);
}

.sidebar-nav__icon {
  font-size: 1rem;
}

.sidebar__divider {
  width: 100%;
  height: 1px;
  background: var(--color-border);
}

.sidebar-overview {
  margin-top: auto;
  padding-top: 0.25rem;
}

.sidebar-overview__list {
  display: grid;
  gap: 0.35rem;
  margin-top: 0.5rem;
}

.sidebar-overview__list p {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  color: var(--color-text-muted);
  font-size: 0.78rem;
  line-height: 1.4;
}

.sidebar-overview__list strong {
  color: var(--color-heading);
  font-weight: 600;
  font-size: 0.78rem;
  font-family: var(--font-mono);
}

.main-content {
  display: grid;
  align-content: start;
  gap: 1rem;
  height: 100vh;
  padding: 1.25rem;
  padding-bottom: 18rem;
  min-width: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.floating-inspector {
  position: fixed;
  left: calc(clamp(14.5rem, 18vw, 16rem) + 1rem);
  right: 1rem;
  bottom: 1rem;
  z-index: 20;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
  box-shadow: var(--shadow-card-hover);
  overflow: hidden;
}

.floating-inspector.is-collapsed {
  box-shadow: var(--shadow-card);
}

.floating-inspector__tab {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 1rem;
  border: 0;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  cursor: pointer;
  text-align: left;
}

.floating-inspector.is-collapsed .floating-inspector__tab {
  border-bottom: 0;
}

.floating-inspector__tab:hover {
  background: var(--color-surface-muted);
}

.floating-inspector__tab-copy {
  min-width: 0;
  display: grid;
  gap: 0.15rem;
}

.floating-inspector__tab-kicker {
  font-size: 0.7rem;
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
  color: var(--color-text-muted);
}

.floating-inspector__tab-copy strong {
  color: var(--color-heading);
  font-size: 0.88rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.floating-inspector__tab-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
  color: var(--color-accent-strong);
  font-size: 0.78rem;
  font-weight: 600;
  font-family: var(--font-mono);
}

.floating-inspector__tab-icon {
  font-size: 1rem;
}

.floating-inspector__body {
  max-height: min(24rem, calc(100vh - 10rem));
  overflow: auto;
}

.floating-inspector-enter-active,
.floating-inspector-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.floating-inspector-enter-from,
.floating-inspector-leave-to {
  opacity: 0;
  transform: translateY(0.75rem);
}

.dialog-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  width: 100%;
}

.dialog-heading h2 {
  margin: 0.15rem 0 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.confirm-delete {
  display: grid;
  gap: 1rem;
}

.confirm-delete__message {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  line-height: 1.6;
}

.confirm-delete__target {
  display: grid;
  gap: 0.25rem;
  padding: 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
}

.confirm-delete__target span {
  color: var(--color-text-muted);
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

.confirm-delete__target strong {
  min-width: 0;
  color: var(--color-heading);
  font-size: var(--font-size-small);
  overflow-wrap: anywhere;
}

.confirm-delete__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  flex-wrap: wrap;
  padding-top: 0.25rem;
}

@media (max-width: 1080px) {
  .app-shell {
    grid-template-columns: minmax(0, 1fr);
    height: auto;
    min-height: 100vh;
    overflow: visible;
  }

  .sidebar {
    height: auto;
    border-right: 0;
    border-bottom: 1px solid var(--color-border);
    overflow: visible;
  }

  .main-content {
    height: auto;
    min-height: 0;
    overflow: visible;
  }

  .floating-inspector {
    left: 1rem;
  }
}

@media (max-width: 720px) {
  .sidebar,
  .main-content {
    padding: 0.875rem;
  }

  .sidebar__brand {
    align-items: flex-start;
  }

  .main-content {
    padding-bottom: 15rem;
  }

  .floating-inspector {
    left: 0.75rem;
    right: 0.75rem;
    bottom: 0.75rem;
  }

  .floating-inspector__tab {
    align-items: flex-start;
    flex-direction: column;
  }

  .floating-inspector__tab-meta {
    width: 100%;
    justify-content: space-between;
  }
}
</style>
