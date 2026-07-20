<script setup lang="ts">
import { computed, defineAsyncComponent, onErrorCaptured, onMounted, onUnmounted, ref, watch, type Ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { filterDownloads } from "./lib/download-filter";

import CategorySidebar from "./components/layout/CategorySidebar.vue";
import DownloadComposer from "./components/limedl/DownloadComposer.vue";
import DownloadQueueTable from "./components/limedl/DownloadQueueTable.vue";
import DetailPanel from "./components/limedl/DetailPanel.vue";
import TopToolbar from "./components/layout/TopToolbar.vue";
import UiButton from "./components/ui/UiButton.vue";
import ConfirmDialog from "./components/ui/ConfirmDialog.vue";
import UiDialog from "./components/ui/UiDialog.vue";
import ErrorBoundary from "./components/ui/ErrorBoundary.vue";

const SettingsPage = defineAsyncComponent({
  loader: () => import("./components/settings/SettingsPage.vue"),
  loadingComponent: {
    template: '<div class="async-loader"><div class="async-loader__spinner"></div></div>',
  },
});

const LabsPage = defineAsyncComponent({
  loader: () => import("./components/labs/LabsPage.vue"),
  loadingComponent: {
    template: '<div class="async-loader"><div class="async-loader__spinner"></div></div>',
  },
});

const SetupWizard = defineAsyncComponent({
  loader: () => import("./components/setup/SetupWizard.vue"),
  loadingComponent: {
    template: '<div class="async-loader"><div class="async-loader__spinner"></div></div>',
  },
});
import { useLimedl } from "./composables/useLimedl";
import type { UseLimedlOptions } from "./composables/useLimedl";
import { useIoBaseline } from "./composables/useIoBaseline";
import { useOverclock } from "./composables/useOverclock";
import { useCategoryCounts } from "./composables/useCategoryCounts";
import { useNotification } from "./composables/useNotification";
import { useI18n } from "./i18n";
import { useAppSettings } from "./composables/useAppSettings";
import { useViewNavigation } from "./composables/useViewNavigation";
import type { PersistablePage } from "./composables/useViewNavigation";
import { useMultiSelect } from "./composables/useMultiSelect";
import { useAppUpdate } from "./composables/useAppUpdate";
import { useNetworkStatus } from "./composables/useNetworkStatus";
import { DEFAULT_VISIBLE_COLUMNS } from "./lib/column-defs";
import NotificationToast from "./components/ui/NotificationToast.vue";
import ModalOverlay from "./components/layout/ModalOverlay.vue";
import type { AppSettings, SortDirection, SortKey } from "./types/settings";
import type { ViewOptions, MultiSelectState } from "./types/download";
import { getAppSettings, saveAppSettings } from "./lib/tauri/settings-api";

// Multi-select refs (declared before limedlOptions closure)
let multiSelectMode = ref(false);
let selectedIds = ref<Set<string>>(new Set());
let showBatchDeleteDialog = ref(false);
let removedDownloadIds = ref<string[]>([]);

const limedlOptions: UseLimedlOptions = {
  onDownloadFailed: (fileName, reason) => {
    notifyError(
      t("messages.downloadFailed", {
        fileName,
        reason,
      }),
    );
  },
  onDownloadsRemoved: (removedIds) => {
    removedDownloadIds.value = removedIds;
    if (selectedIds.value.size === 0) return;
    let changed = false;
    const next = new Set(selectedIds.value);
    for (const id of removedIds) {
      if (next.has(id)) {
        next.delete(id);
        changed = true;
      }
    }
    if (changed) {
      selectedIds.value = next;
    }
  },
};

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
  isPickingTorrent,
  isRefreshingList: _isRefreshingList,
  isRefreshingStatus,
  isStarting,
  applyAppSettingsDefaults,
  pickDestinationDirectory,
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
  runPauseAll,
  runResumeAll,
  runClearCompleted,
  runBatchDelete,
  selectDownload,
  selectedId,
  selectedSnapshot,
  selectedSummary,
  submitStart,
  autoFillFromClipboard,
  setNotificationsEnabled,
} = useLimedl(limedlOptions);

const { categoryCounts, sidebarStats } = useCategoryCounts(downloads);

const ms = useMultiSelect(downloads as Ref<Array<{ id: string }>>);
// Reassign refs to the composable's refs so closure captures work correctly
multiSelectMode = ms.multiSelectMode;
selectedIds = ms.selectedIds;
showBatchDeleteDialog = ms.showBatchDeleteDialog;
removedDownloadIds = ms.removedDownloadIds;
const {
  handleToggleMultiSelectMode,
  handleToggleSelect,
  handleSelectAll,
  handleDeselectAll,
  handleBatchDelete,
} = ms;

const { t } = useI18n();
const {
  gameMode,
  bufferUsageBytes,
  bufferLimitBytes,
  activeSlots,
  maxSlots,
  queuedCount,
  setGameMode,
} = useIoBaseline();
const { overclockMode, setOverclockMode } = useOverclock();
const showComposerDialog = ref(false);
const activeCategory = ref("");
const searchQuery = ref("");
const sortKey = ref<SortKey>("added_at");
const sortDirection = ref<SortDirection>("desc");
const compactView = ref(false);
const visibleColumns = ref<string[]>([...DEFAULT_VISIBLE_COLUMNS]);
const pendingPermanentDeleteId = ref<string | null>(null);
const settingsPageRef = ref<PersistablePage | null>(null);
const labsPageRef = ref<PersistablePage | null>(null);

const { appSettings, applyAppearanceSettings } = useAppSettings({
  sortKey,
  sortDirection,
  compactView,
  visibleColumns,
  applyAppSettingsDefaults,
  setNotificationsEnabled,
});

// ── Setup wizard integration ──
const showSetupWizard = ref<boolean | null>(null);
const setupInitialSettings = ref<AppSettings | null>(null);
const appVersion = ref("");

const setupStartStep = computed(() => {
  const lastStep = setupInitialSettings.value?.lastSetupStep;
  if (lastStep != null && !setupInitialSettings.value?.setupCompleted) {
    return lastStep;
  }
  return 0;
});

// Check localStorage cache first for instant decision
const cachedSetupDone = localStorage.getItem("limedl.setupCompleted");
if (cachedSetupDone === "true") {
  showSetupWizard.value = false;
}

function checkSetupState() {
  if (appSettings.value) {
    if (appSettings.value.setupCompleted) {
      localStorage.setItem("limedl.setupCompleted", "true");
      showSetupWizard.value = false;
    } else if (showSetupWizard.value === null) {
      setupInitialSettings.value = appSettings.value;
      showSetupWizard.value = true;
    }
  }
}

watch(appSettings, checkSetupState);

async function handleSetupCompleted(settings: AppSettings) {
  // Cache in localStorage for fast boot
  localStorage.setItem("limedl.setupCompleted", "true");
  // Update the global appSettings
  appSettings.value = settings;
  // Apply appearance settings (theme, color mode) to document
  applyAppearanceSettings(settings);
  // Apply download defaults (auto-fill composer)
  applyAppSettingsDefaults(settings);
  // Notifications
  setNotificationsEnabled(settings.notifications?.enabled ?? true);
  // Hide wizard, show main app
  showSetupWizard.value = false;
}

async function handleSetupClosed() {
  // User closed wizard without completing (Escape key).
  // Reload settings from disk since wizard may have written partial changes.
  try {
    const updated = await getAppSettings();
    appSettings.value = updated;
    applyAppearanceSettings(updated);
  } catch {
    // If reload fails, keep the stale value — better than crashing
  }
  showSetupWizard.value = false;
}

async function handleRestartSetup() {
  const currentSettings = appSettings.value;
  if (!currentSettings) return;

  // Restart uses the last-persisted settings as the wizard's starting point.
  // Unsaved draft changes in the settings panel are intentionally discarded —
  // the user is explicitly choosing to re-run the setup wizard.
  localStorage.removeItem("limedl.setupCompleted");
  currentSettings.setupCompleted = false;
  currentSettings.lastSetupStep = null;

  try {
    await saveAppSettings(currentSettings);
  } catch (e) {
    console.error("Failed to reset setup state:", e);
  }

  setupInitialSettings.value = { ...currentSettings };
  showSetupWizard.value = true;
}
// ── End setup wizard integration ──

const {
  currentView,
  settingsHasUnsavedChanges,
  labsHasUnsavedChanges,
  isSavingBeforeNavigation,
  navigateTo,
  cancelDiscardSettings,
  confirmDiscardSettings,
  saveSettingsAndNavigate,
  showUnsavedSettingsDialog,
  unsavedDialogKicker,
  unsavedDialogTitle,
  unsavedDialogMessage,
} = useViewNavigation({
  settingsPageRef,
  labsPageRef,
});

const filteredDownloads = computed(() =>
  filterDownloads(downloads.value, searchQuery.value, activeCategory.value),
);

onErrorCaptured((err, _instance, info) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error("[Component Error]", err, info);
  useNotification().notify(`Error: ${message}`, "error");
  // Return false to prevent error from propagating further
  return false;
});

const { notifications, notifyError, dismiss } = useNotification();

const { updateAvailable, runStartupCheck } = useAppUpdate();

onMounted(() => {
  runStartupCheck();
  checkSetupState();

  // Fetch real app version from Tauri metadata
  getVersion().then((v) => { appVersion.value = v; }).catch(() => {});

  // Safety: if settings never load (backend crash, IPC failure),
  // bail out to the main app after 5 seconds instead of showing an infinite spinner
  setTimeout(() => {
    if (showSetupWizard.value === null) {
      console.warn("Settings never loaded, showing main app as fallback");
      showSetupWizard.value = false;
    }
  }, 5000);

  // ── Network & connection monitoring ──
  // Browser online/offline detection (works in all modes)
  const networkStatus = useNetworkStatus();
  networkStatus.start();
  onUnmounted(() => networkStatus.stop());

  // WebSocket reconnection monitoring (NAS mode only)
  // Shows toast when the WS link drops / reconnects
  if (import.meta.env.MODE === 'nas') {
    import('./lib/ws/ws-invoke').then(({ connectionStatus }) => {
      // eslint-disable-next-line vue/no-setup-props-destructure
      watch(connectionStatus, (status, prev) => {
        if (status === 'reconnecting' && prev !== 'reconnecting') {
          useNotification().notifyWarning(t('messages.connectionLost'), 10000);
        } else if (status === 'connected' && prev === 'reconnecting') {
          useNotification().notifySuccess(t('messages.connectionRestored'));
        }
      });
    });
  }
});

const selectedOverview = computed(() => selectedSnapshot.value ?? selectedSummary.value);

const showDetailInfo = computed(() => appSettings.value?.appearance?.showDetailInfo ?? true);

const viewOptions = computed<ViewOptions>(() => ({
  sortKey: sortKey.value,
  sortDirection: sortDirection.value,
  compactView: compactView.value,
  visibleColumns: visibleColumns.value,
}));

const multiSelectState = computed<MultiSelectState>(() => ({
  multiSelectMode: multiSelectMode.value,
  selectedIds: selectedIds.value,
  removedDownloadIds: removedDownloadIds.value,
}));

const pendingPermanentDeleteTask = computed(
  () => downloads.value.find((download) => download.id === pendingPermanentDeleteId.value) ?? null,
);

const btStatusData = computed(() => {
  if (!btRuntimeStatus.value) return null;
  return {
    dhtNodes: btRuntimeStatus.value.dhtNodes ?? 0,
    uploadSpeed: btRuntimeStatus.value.uploadSpeedBytesPerSecond ?? 0,
    peers: btRuntimeStatus.value.peerCount ?? 0,
    torrents: btRuntimeStatus.value.torrentCount ?? 0,
  };
});

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

function handleDelete() {
  if (selectedId.value) {
    runDeleteTask(selectedId.value);
  }
}

function handleRefresh() {
  void refreshList();
}

async function handleToggleGameMode() {
  await setGameMode(!gameMode.value);
}

async function handleToggleOverclockMode() {
  await setOverclockMode(!overclockMode.value);
}

async function confirmBatchDelete() {
  const ids = [...selectedIds.value];
  showBatchDeleteDialog.value = false;
  if (ids.length === 0) return;
  await runBatchDelete(ids);
  selectedIds.value = new Set();
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

watch(
  () => showComposerDialog.value,
  (isOpen) => {
    if (!isOpen || !appSettings.value) {
      return;
    }

    applyAppSettingsDefaults(appSettings.value);
    autoFillFromClipboard();
  },
);
</script>

<template>
  <!-- Loading state while determining setup state -->
  <template v-if="showSetupWizard === null">
    <div class="app-loading">
      <div class="app-loading__spinner i-ri-loader-4-line" aria-hidden="true" />
    </div>
  </template>

  <!-- Setup wizard -->
  <template v-else-if="showSetupWizard">
    <Transition name="wizard">
      <SetupWizard
        :app-version="appVersion || undefined"
        :initial-settings="setupInitialSettings ?? undefined"
        :start-from-step="setupStartStep"
        @completed="handleSetupCompleted"
        @close="handleSetupClosed"
      />
    </Transition>
  </template>

  <!-- Normal app layout -->
  <div v-else class="app-root">
    <NotificationToast :notifications="notifications" @dismiss="dismiss" />

    <!-- Top toolbar (only show on home view) -->
    <TopToolbar
      v-if="currentView === 'home'"
      :search-query="searchQuery"
      :has-selection="!!selectedId"
      :bt-status="btStatusData"
      :sort-key="sortKey"
      :sort-direction="sortDirection"
      :compact-view="compactView"
      :visible-columns="visibleColumns"
      :multi-select-mode="multiSelectMode"
      :selected-count="selectedIds.size"
      :filtered-count="filteredDownloads.length"
      :game-mode="gameMode"
      :game-mode-buffer-mb="appSettings?.ioBaseline?.gameModeBufferMb"
      :overclock-mode="overclockMode"
      @update:search-query="searchQuery = $event"
      @update:sort-key="sortKey = $event"
      @update:sort-direction="sortDirection = $event"
      @update:compact-view="compactView = $event"
      @update:visible-columns="visibleColumns = $event"
      @add-task="showComposerDialog = true"
      @delete="handleDelete"
      @refresh="handleRefresh"
      @update:multi-select-mode="handleToggleMultiSelectMode"
      @pause-all="runPauseAll"
      @resume-all="runResumeAll"
      @clear-completed="runClearCompleted"
      @select-all="handleSelectAll"
      @deselect-all="handleDeselectAll"
      @batch-delete="handleBatchDelete"
      @toggle-game-mode="handleToggleGameMode"
      @toggle-overclock-mode="handleToggleOverclockMode"
    />

    <!-- Main layout: sidebar + content -->
    <div class="app-body">
      <CategorySidebar
        :active-category="activeCategory"
        :current-view="currentView"
        :counts="categoryCounts as unknown as Record<string, number>"
        :stats="sidebarStats"
        :update-available="updateAvailable"
        @update:active-category="activeCategory = $event"
        @navigate="navigateTo"
      />

      <main class="content-area">
        <!-- Home view: table + detail panel -->
        <template v-if="currentView === 'home'">
          <div class="table-wrapper">
            <ErrorBoundary>
              <DownloadQueueTable
                :downloads="filteredDownloads"
                :selected-id="selectedId"
                :task-action-name="actionName"
                :is-auto-refreshing="isAutoRefreshing"
                :view-options="viewOptions"
                :multi-select="multiSelectState"
                @copy-link="runCopyLink"
                @delete-task="runDeleteTask"
                @delete-task-permanently="requestPermanentDelete"
                @open-in-explorer="runOpenInExplorer"
                @pause-or-resume="handleTaskPauseOrResume"
                @select="selectDownload"
                @toggle-select="handleToggleSelect"
              />
            </ErrorBoundary>
          </div>

          <!-- Collapsible bottom detail panel -->
          <ErrorBoundary>
            <DetailPanel
              v-if="selectedId"
              :selected-overview="selectedOverview as import('./types/download').DownloadSummary | null"
              :selected-snapshot="selectedSnapshot"
              :selected-id="selectedId"
              :can-pause="canPause"
              :can-resume="canResume"
              :can-cancel="canCancel"
              :action-name="actionName"
              :is-refreshing-status="isRefreshingStatus"
              :show-detail-info="showDetailInfo"
              @close="selectDownload(null)"
              @refresh="handleRefreshSelected"
              @pause="runPause"
              @resume="runResume"
              @cancel="runCancel"
            />
          </ErrorBoundary>
        </template>
      </main>
    </div>

    <!-- Settings & Labs as centered modal overlays -->
    <ModalOverlay :model-value="currentView === 'settings'" @close="navigateTo('home')">
      <ErrorBoundary>
        <SettingsPage
          ref="settingsPage"
          :settings="appSettings"
          :game-mode="gameMode"
          :buffer-usage-bytes="bufferUsageBytes"
          :buffer-limit-bytes="bufferLimitBytes"
          :active-slots="activeSlots"
          :max-slots="maxSlots"
          :queued-count="queuedCount"
          @dirty-change="handleSettingsDirtyChange"
          @saved="handleSettingsSaved"
          @restart-setup="handleRestartSetup"
        />
      </ErrorBoundary>
    </ModalOverlay>
    <ModalOverlay :model-value="currentView === 'labs'" @close="navigateTo('home')">
      <ErrorBoundary>
        <LabsPage
          ref="labsPage"
          :settings="appSettings"
          @dirty-change="handleLabsDirtyChange"
          @saved="handleLabsSaved"
        />
      </ErrorBoundary>
    </ModalOverlay>

    <!-- Dialogs -->
    <UiDialog v-model="showComposerDialog" width="min(46rem, calc(100vw - 1.5rem))" :close-on-overlay="false">
      <template #title>
        <div class="dialog-heading dialog-heading--inline">
          <span class="dialog-heading__icon i-ri-download-cloud-2-line" aria-hidden="true" />
          <h2>{{ t("dialog.newTaskTitle") }}</h2>
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

    <ConfirmDialog
      :model-value="Boolean(pendingPermanentDeleteId)"
      :kicker="t('dialog.confirmDelete')"
      :title="t('dialog.permanentDeleteTitle')"
      :message="t('dialog.permanentDeleteMessage')"
      :confirm-text="
        actionName === 'Purge' ? t('dialog.deleting') : t('dialog.confirmPermanentDelete')
      "
      :cancel-text="t('common.cancel')"
      icon="i-ri-delete-bin-line"
      :icon-danger="true"
      confirm-icon="i-ri-delete-bin-line"
      :confirm-loading="actionName === 'Purge'"
      :cancel-disabled="actionName === 'Purge'"
      :close-on-overlay="actionName !== 'Purge'"
      @cancel="cancelPermanentDelete"
      @confirm="confirmPermanentDelete"
    >
      <div v-if="pendingPermanentDeleteTask" class="confirm-delete__target">
        <span>{{ t("dialog.targetFile") }}</span>
        <strong>{{ pendingPermanentDeleteTask.fileName }}</strong>
      </div>
    </ConfirmDialog>

    <ConfirmDialog
      :model-value="showUnsavedSettingsDialog"
      :kicker="unsavedDialogKicker"
      :title="unsavedDialogTitle"
      :message="unsavedDialogMessage"
      icon="i-ri-error-warning-line"
      :confirm-text="
        isSavingBeforeNavigation ? t('common.saving') : t('dialog.saveSettingsAndLeave')
      "
      :cancel-text="t('dialog.keepEditing')"
      confirm-variant="primary"
      confirm-icon="i-ri-save-line"
      :confirm-loading="isSavingBeforeNavigation"
      @cancel="cancelDiscardSettings"
      @confirm="saveSettingsAndNavigate"
    >
      <template #extra-actions>
        <UiButton
          type="button"
          variant="danger"
          icon="i-ri-arrow-right-line"
          @click="confirmDiscardSettings"
        >
          {{ t("dialog.discardSettings") }}
        </UiButton>
      </template>
    </ConfirmDialog>

    <ConfirmDialog
      :model-value="showBatchDeleteDialog"
      :kicker="t('dialog.confirmDelete')"
      :title="t('dialog.batchDeleteTitle')"
      :message="t('dialog.batchDeleteMessage', { count: selectedIds.size })"
      :confirm-text="t('dialog.confirmBatchDelete')"
      :cancel-text="t('common.cancel')"
      confirm-icon="i-ri-delete-bin-line"
      @cancel="showBatchDeleteDialog = false"
      @confirm="confirmBatchDelete"
    />
  </div>
</template>

<style scoped>
/* ── Root layout ── */
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.table-wrapper {
  flex: 1;
  overflow: auto;
  min-height: 0;
  padding: var(--space-4);
}

/* ── Dialog styles ── */

.dialog-heading--inline {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.dialog-heading--inline h2 {
  margin: 0;
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

/* ── Loading / splash screen ── */
.app-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: var(--color-bg-base);
}

.app-loading__spinner {
  font-size: 2.5rem;
  color: var(--color-accent);
  animation: app-loading-spin 1s linear infinite;
}

@keyframes app-loading-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .app-loading__spinner {
    animation: none;
  }
}

/* ── Wizard exit transition (entrance handled internally by SetupWizard) ── */
.wizard-leave-active {
  transition: opacity 250ms ease, transform 250ms ease;
}

.wizard-leave-to {
  opacity: 0;
  transform: scale(0.95);
}

@media (prefers-reduced-motion: reduce) {
  .wizard-leave-active {
    transition: none;
  }
}
</style>

<style>
/* ── Async component loading spinner (global — used by defineAsyncComponent loadingComponent) ── */
.async-loader {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 3rem;
}

.async-loader__spinner {
  width: 1.5rem;
  height: 1.5rem;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: async-loader-spin 0.8s linear infinite;
}

@keyframes async-loader-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .async-loader__spinner {
    animation: none;
  }
}
</style>
