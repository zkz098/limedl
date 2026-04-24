<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

import DownloadComposer from "./components/downloader/DownloadComposer.vue";
import DownloadInspector from "./components/downloader/DownloadInspector.vue";
import DownloadQueueTable from "./components/downloader/DownloadQueueTable.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import UiButton from "./components/ui/UiButton.vue";
import UiDialog from "./components/ui/UiDialog.vue";
import { formatSpeed } from "./lib/download-format";
import { getAppSettings } from "./lib/tauri/settings-api";
import { useDownloader } from "./composables/useDownloader";
import { useI18n } from "./i18n";
import type { AppSettings } from "./types/settings";

const {
  actionName,
  canCancel,
  canPause,
  canResume,
  canPauseDownload,
  canResumeDownload,
  downloads,
  errorMessage,
  form,
  infoMessage,
  isAutoRefreshing,
  isPickingDirectory,
  isPickingTorrent,
  isRefreshingList,
  isRefreshingStatus,
  isStarting,
  applyAppSettingsDefaults,
  pickDestinationDirectory,
  pickTorrentSourceFile,
  refreshList,
  refreshStatus,
  runCancel,
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
const currentView = ref<"home" | "settings">("home");
const appSettings = ref<AppSettings | null>(null);
const pendingPermanentDeleteId = ref<string | null>(null);
const pendingView = ref<"home" | "settings" | null>(null);
const settingsHasUnsavedChanges = ref(false);
const notificationMessage = ref("");
const knownFailedDownloadIds = new Set<string>();
let notificationTimer: ReturnType<typeof setTimeout> | null = null;
let hasSeenInitialDownloadList = false;

const selectedOverview = computed(() => selectedSnapshot.value ?? selectedSummary.value);
const showUnsavedSettingsDialog = computed(() => pendingView.value !== null);
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

function showNotification(message: string) {
  notificationMessage.value = message;
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
  notificationTimer = setTimeout(() => {
    notificationMessage.value = "";
    notificationTimer = null;
  }, 3600);
}

const handleSubmitStart = async () => {
  await submitStart();
  if (!errorMessage.value) {
    showComposerDialog.value = false;
  }
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

function navigateTo(view: "home" | "settings") {
  if (view === currentView.value) {
    return;
  }

  if (currentView.value === "settings" && settingsHasUnsavedChanges.value) {
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

  if (nextView) {
    currentView.value = nextView;
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
  applyAppSettingsDefaults(nextSettings);
}

function handleSettingsDirtyChange(isDirty: boolean) {
  settingsHasUnsavedChanges.value = isDirty;
}

async function loadSettings() {
  try {
    appSettings.value = await getAppSettings();
    applyAppSettingsDefaults(appSettings.value);
  } catch (error) {
    console.error("Failed to load app settings", error);
  }
}

onMounted(() => {
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
      showNotification(
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
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
});
</script>

<template>
  <main class="app-shell min-h-screen text-[var(--color-text-main)]">
    <div class="app-shell__backdrop" aria-hidden="true" />

    <Transition name="app-notification">
      <div v-if="notificationMessage" class="app-notification" role="alert">
        <span class="i-ri-error-warning-line" aria-hidden="true" />
        <span>{{ notificationMessage }}</span>
      </div>
    </Transition>

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
    </aside>

    <section class="main-content">
      <DownloadQueueTable
        v-if="currentView === 'home'"
        :downloads="downloads"
        :error-message="errorMessage"
        :info-message="infoMessage"
        :is-auto-refreshing="isAutoRefreshing"
        :is-refreshing-list="isRefreshingList"
        :selected-id="selectedId"
        :task-action-name="actionName"
        @delete-task="runDeleteTask"
        @delete-task-permanently="requestPermanentDelete"
        @open-in-explorer="runOpenInExplorer"
        @pause-or-resume="handleTaskPauseOrResume"
        @refresh="refreshList"
        @select="selectDownload"
      />
      <SettingsPage
        v-else
        :settings="appSettings"
        @dirty-change="handleSettingsDirtyChange"
        @saved="handleSettingsSaved"
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
        :is-picking-torrent="isPickingTorrent"
        :is-starting="isStarting"
        :settings="appSettings"
        @pick-directory="pickDestinationDirectory"
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
            <p class="section-kicker">{{ t("settings.kicker") }}</p>
            <h2>{{ t("dialog.unsavedSettingsTitle") }}</h2>
          </div>
          <span class="dialog-heading__icon i-ri-error-warning-line" aria-hidden="true" />
        </div>
      </template>

      <div class="confirm-delete">
        <p class="confirm-delete__message">
          {{ t("dialog.unsavedSettingsMessage") }}
        </p>
        <div class="confirm-delete__actions">
          <UiButton type="button" variant="secondary" @click="cancelDiscardSettings">
            {{ t("dialog.keepEditing") }}
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
  position: relative;
  display: grid;
  grid-template-columns: minmax(14.5rem, 16rem) minmax(0, 1fr);
  height: 100vh;
  overflow: hidden;
  background:
    radial-gradient(
      circle at top left,
      color-mix(in srgb, var(--color-accent-soft) 100%, transparent) 0,
      transparent 26rem
    ),
    linear-gradient(180deg, var(--color-bg-base), var(--color-bg-base));
}

.app-shell__backdrop {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(
      circle at 85% 12%,
      color-mix(in srgb, var(--color-accent) 10%, transparent) 0,
      transparent 18rem
    ),
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--color-panel) 75%, transparent),
      transparent 18rem
    );
}

.sidebar,
.main-content {
  position: relative;
  z-index: 1;
}

.app-notification {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 50;
  max-width: min(30rem, calc(100vw - 2rem));
  display: inline-flex;
  align-items: flex-start;
  gap: 0.55rem;
  padding: 0.8rem 0.9rem;
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) 96%, transparent);
  box-shadow: var(--shadow-card-hover);
  color: var(--color-danger-text);
  font-size: 0.85rem;
  line-height: 1.45;
  backdrop-filter: blur(0.875rem);
}

.app-notification span:first-child {
  flex: 0 0 auto;
  margin-top: 0.1rem;
  font-size: 1rem;
}

.app-notification-enter-active,
.app-notification-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.app-notification-enter-from,
.app-notification-leave-to {
  opacity: 0;
  transform: translateY(-0.45rem);
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  height: 100vh;
  padding: 1.25rem;
  border-right: 1px solid var(--color-border);
  background: color-mix(in srgb, var(--color-panel-muted) 82%, transparent);
  backdrop-filter: blur(0.875rem);
  overflow: hidden;
}

.sidebar__brand h1 {
  margin: 0.2rem 0 0;
  font-family: var(--font-display);
  font-size: 2rem;
  line-height: 1.08;
  color: var(--color-heading);
}

.sidebar__brand {
  display: flex;
  align-items: center;
  gap: 0.875rem;
}

.sidebar__logo-mark {
  width: 3rem;
  height: 3rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 0.875rem;
  background: linear-gradient(135deg, var(--color-accent), var(--color-accent-alt));
  color: var(--color-accent-contrast);
  box-shadow: var(--shadow-accent);
  font-size: 1.4rem;
}

.panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.875rem;
}

.panel-head__icon,
.dialog-heading__icon {
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

.dialog-heading__icon--danger {
  color: var(--color-danger-text);
  background: var(--color-danger-bg);
  border-color: var(--color-danger-border);
}

.sidebar-nav {
  display: grid;
  gap: 0.35rem;
}

.sidebar-nav__item {
  width: 100%;
  min-height: 2.75rem;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0 0.85rem;
  border: 1px solid transparent;
  border-radius: 0.75rem;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  text-align: left;
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    color 0.2s ease,
    transform 0.2s ease;
}

.sidebar-nav__item:hover {
  background: color-mix(in srgb, var(--color-accent-soft) 36%, var(--color-panel));
  color: var(--color-heading);
  transform: translateX(0.125rem);
}

.sidebar-nav__item--active {
  background: color-mix(in srgb, var(--color-accent-soft) 55%, var(--color-panel));
  border-color: color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
  color: var(--color-accent-strong);
}

.sidebar-nav__icon {
  font-size: 1rem;
}

.sidebar__divider {
  width: 100%;
  height: 1px;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--color-border) 0%, transparent) 0,
    var(--color-border) 16%,
    var(--color-border) 84%,
    color-mix(in srgb, var(--color-border) 0%, transparent) 100%
  );
}

.sidebar-overview {
  margin-top: auto;
  padding-top: 0.25rem;
}

.sidebar-overview__list {
  display: grid;
  gap: 0.4rem;
  margin-top: 0.55rem;
}

.sidebar-overview__list p {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  color: var(--color-text-muted);
  font-size: 0.8rem;
  line-height: 1.4;
}

.sidebar-overview__list strong {
  color: var(--color-heading);
  font-weight: 600;
  font-size: 0.8rem;
}

.main-content {
  display: grid;
  align-content: start;
  gap: 1rem;
  height: 100vh;
  padding: 1.25rem 1.25rem 19rem;
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
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) 94%, transparent);
  box-shadow: var(--shadow-card-hover);
  backdrop-filter: blur(1rem);
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
  padding: 0.85rem 1rem;
  border: 0;
  border-bottom: 1px solid var(--color-border);
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--color-accent-soft) 60%, var(--color-panel)) 0,
    var(--color-panel) 100%
  );
  cursor: pointer;
  text-align: left;
}

.floating-inspector.is-collapsed .floating-inspector__tab {
  border-bottom: 0;
}

.floating-inspector__tab-copy {
  min-width: 0;
  display: grid;
  gap: 0.15rem;
}

.floating-inspector__tab-kicker {
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-text-muted);
}

.floating-inspector__tab-copy strong {
  color: var(--color-heading);
  font-size: 0.92rem;
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
  font-size: 0.8rem;
  font-weight: 600;
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
    opacity 0.22s ease,
    transform 0.22s ease;
}

.floating-inspector-enter-from,
.floating-inspector-leave-to {
  opacity: 0;
  transform: translateY(1rem);
}

.dialog-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  width: 100%;
}

.dialog-heading h2 {
  margin: 0.25rem 0 0;
  font-size: 1.25rem;
  color: var(--color-heading);
}

.confirm-delete {
  display: grid;
  gap: 1rem;
}

.confirm-delete__message {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.9rem;
  line-height: 1.6;
}

.confirm-delete__target {
  display: grid;
  gap: 0.25rem;
  padding: 0.85rem 0.95rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
}

.confirm-delete__target span {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.confirm-delete__target strong {
  min-width: 0;
  color: var(--color-heading);
  font-size: 0.95rem;
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
    padding-bottom: 16rem;
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
