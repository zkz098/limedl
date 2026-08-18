<script setup lang="ts">
import {
  computed,
  defineAsyncComponent,
  onErrorCaptured,
  onMounted,
  onUnmounted,
  ref,
  watch,
  type Ref,
} from "vue";
import { storeToRefs } from "pinia";
import { filterDownloads } from "./lib/download-filter";

import CategorySidebar from "./components/layout/CategorySidebar.vue";
import DownloadComposer from "./components/limedl/DownloadComposer.vue";
import DownloadQueueTable from "./components/limedl/DownloadQueueTable.vue";
import DetailPanel from "./components/limedl/DetailPanel.vue";
import TopToolbar from "./components/layout/TopToolbar.vue";
import UiButton from "./components/ui/UiButton.vue";
import BtSpeedLimitModal from "./components/limedl/BtSpeedLimitModal.vue";
import BatchActionBar from "./components/limedl/BatchActionBar.vue";
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
import { useDownloadStore } from "./stores/download";
import { useNotificationStore } from "./stores/notification";
import { useAppSettingsStore } from "./stores/appSettings";
import { useIoBaseline } from "./composables/useIoBaseline";
import { useSetupWizardLifecycle } from "./composables/useSetupWizardLifecycle";
import { useOverclock } from "./composables/useOverclock";
import { useCategoryCounts } from "./composables/useCategoryCounts";
import { useI18n } from "./i18n";
import { useViewNavigation } from "./composables/useViewNavigation";
import type { PersistablePage } from "./composables/useViewNavigation";
import { useMultiSelect } from "./composables/useMultiSelect";
import { useNetworkStatusStore } from "./stores/networkStatus";
import { useAppUpdateStore } from "./stores/appUpdate";
import NotificationToast from "./components/ui/NotificationToast.vue";
import ModalOverlay from "./components/layout/ModalOverlay.vue";
import type { AppSettings } from "./types/settings";
import type { ViewOptions, MultiSelectState } from "./types/download";
import { saveAppSettings } from "./lib/tauri/settings-api";
import { openDownloadDir, openDownloadFile, setBtSpeedLimit } from "./lib/tauri/download-api";
import { toMessage, toErrorMessage } from "./composables/downloadHelpers";

// Multi-select refs (declared before configure closure)
let multiSelectMode = ref(false);
let selectedIds = ref<Set<string>>(new Set());
let showBatchDeleteDialog = ref(false);
let removedDownloadIds = ref<string[]>([]);

// BT speed limit modal state
const showBtSpeedLimitModal = ref(false);
const btSpeedLimitTaskId = ref("");
const btSpeedLimitDownloadLimit = ref(0);
const btSpeedLimitUploadLimit = ref(0);

// ── Pinia stores ───────────────────────────────────────────────────
const notify = useNotificationStore();
const downloadStore = useDownloadStore();
downloadStore.configure({
  onDownloadFailed: (fileName, reason) => {
    notify.notifyError(
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
});

// Use storeToRefs to preserve ref reactivity (Pinia auto-unwraps otherwise)
const {
  actionName,
  canCancel,
  canPause,
  canResume,
  btRuntimeStatus,
  downloads,
  isAutoRefreshing,
  isRefreshingList: _isRefreshingList,
  isRefreshingStatus,
  selectedId,
  selectedSnapshot,
  selectedSummary,
} = storeToRefs(downloadStore);

const {
  applyAppSettingsDefaults,
  canPauseDownload,
  canResumeDownload,
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
  runBatchPause,
  runBatchResume,
  runBatchCancel,
  runSetPriority,
  selectDownload,
  autoFillFromClipboard,
  setNotificationsEnabled,
  setMessage,
  setError,
} = downloadStore;

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
const pendingPermanentDeleteId = ref<string | null>(null);
const settingsPageRef = ref<PersistablePage | null>(null);
const labsPageRef = ref<PersistablePage | null>(null);

const appSettingsStore = useAppSettingsStore();
const { appSettings, sortKey, sortDirection, compactView, visibleColumns } =
  storeToRefs(appSettingsStore);
const { applyAppearanceSettings } = appSettingsStore;

const {
  showSetupWizard,
  setupInitialSettings,
  appVersion,
  setupStartStep,
  handleSetupCompleted,
  handleSetupClosed,
  handleRestartSetup,
  mountSetupWizard,
} = useSetupWizardLifecycle({
  appSettings,
  applyAppearanceSettings,
  applyAppSettingsDefaults,
  setNotificationsEnabled,
});

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
  const message = toErrorMessage(err);
  console.error("[Component Error]", err, info);
  notify.notify(`Error: ${message}`, "error");
  // Return false to prevent error from propagating further
  return false;
});

const appUpdateStore = useAppUpdateStore();
const { updateAvailable } = storeToRefs(appUpdateStore);

onMounted(() => {
  appUpdateStore.runStartupCheck();
  mountSetupWizard();

  // Initialize Pinia stores (replaces onMounted from composables)
  downloadStore.initStore();
  appSettingsStore.initStore();

  // ── Network & connection monitoring ──
  // Browser online/offline detection (works in all modes)
  const networkStatus = useNetworkStatusStore();
  networkStatus.start();

  // WebSocket reconnection monitoring (NAS mode only)
  // Shows toast when the WS link drops / reconnects
  if (import.meta.env.MODE === "nas") {
    import("./lib/ws/ws-invoke").then(({ connectionStatus }) => {
      // eslint-disable-next-line vue/no-setup-props-destructure
      watch(connectionStatus, (status, prev) => {
        if (status === "reconnecting" && prev !== "reconnecting") {
          notify.notifyWarning(t("messages.connectionLost"), 10000);
        } else if (status === "connected" && prev === "reconnecting") {
          notify.notifySuccess(t("messages.connectionRestored"));
        }
      });
    });
  }
});

onUnmounted(() => {
  downloadStore.destroyStore();
  appSettingsStore.destroyStore();
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

const selectedDownloadItems = computed(() =>
  downloads.value.filter((d) => selectedIds.value.has(d.id)),
);

const batchBarStats = computed(() => {
  const items = selectedDownloadItems.value;
  return {
    canPauseCount: items.filter((d) => canPauseDownload(d)).length,
    canResumeCount: items.filter((d) => canResumeDownload(d)).length,
    canCancelCount: items.filter((d) => !["completed", "failed", "canceled"].includes(d.state))
      .length,
  };
});

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

const handleDoubleClick = async (downloadId: string) => {
  const target = downloads.value.find((d) => d.id === downloadId);
  if (!target || !appSettings.value?.doubleClick) return;

  const isCompleted = target.state === "completed";
  const dc = appSettings.value.doubleClick;

  if (isCompleted) {
    switch (dc.onCompleted) {
      case "none":
        break;
      case "open_file":
        try {
          await openDownloadFile(downloadId);
          setMessage(t("messages.openedFile"));
        } catch (error) {
          setError(toMessage(error));
        }
        break;
      case "open_in_explorer":
        await runOpenInExplorer(downloadId);
        break;
      case "open_download_dir":
        try {
          await openDownloadDir(downloadId);
          setMessage(t("messages.openedDownloadDir"));
        } catch (error) {
          setError(toMessage(error));
        }
        break;
    }
  } else {
    switch (dc.onUncompleted) {
      case "toggle_pause_resume":
        await handleTaskPauseOrResume(downloadId);
        break;
      case "none":
        break;
    }
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
  const newMode = !gameMode.value;
  await setGameMode(newMode);
  if (newMode && appSettings.value) {
    const updated = { ...appSettings.value };
    updated.scheduler = {
      ...updated.scheduler,
      mode: "automatic",
      automatic: {
        ...updated.scheduler.automatic,
        adaptiveProfile: "conservative",
      },
    };
    await saveAppSettings(updated);
  }
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

async function handleBatchPause() {
  const ids = [...selectedIds.value];
  if (ids.length === 0) return;
  await runBatchPause(ids);
}

async function handleBatchResume() {
  const ids = [...selectedIds.value];
  if (ids.length === 0) return;
  await runBatchResume(ids);
}

async function handleBatchCancel() {
  const ids = [...selectedIds.value];
  if (ids.length === 0) return;
  await runBatchCancel(ids);
  selectedIds.value = new Set();
  multiSelectMode.value = false;
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

// ── BT speed limit modal ──

function handleSetBtSpeedLimit(downloadId: string) {
  const download = downloads.value.find((d) => d.id === downloadId);
  if (!download) return;

  btSpeedLimitTaskId.value = downloadId;
  btSpeedLimitDownloadLimit.value = download.downloadLimitBps ?? 0;
  btSpeedLimitUploadLimit.value = download.uploadLimitBps ?? 0;
  showBtSpeedLimitModal.value = true;
}

async function handleBtSpeedLimitConfirm(payload: {
  taskId: string;
  downloadLimit: number;
  uploadLimit: number;
}) {
  try {
    await setBtSpeedLimit(payload.taskId, payload.downloadLimit, payload.uploadLimit);
    showBtSpeedLimitModal.value = false;
    notify.notifySuccess(t("messages.speedLimitUpdated"));
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    notify.notifyError(t("messages.speedLimitError", { error: message }));
  }
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
    <NotificationToast :notifications="notify.notifications" @dismiss="notify.dismiss" />

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
          <BatchActionBar
            :selected-count="selectedIds.size"
            :multi-select-mode="multiSelectMode"
            :can-pause-count="batchBarStats.canPauseCount"
            :can-resume-count="batchBarStats.canResumeCount"
            :can-cancel-count="batchBarStats.canCancelCount"
            @pause="handleBatchPause"
            @resume="handleBatchResume"
            @cancel="handleBatchCancel"
            @clear-selection="handleDeselectAll"
          />
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
                @double-click="handleDoubleClick"
                @open-in-explorer="runOpenInExplorer"
                @pause-or-resume="handleTaskPauseOrResume"
                @select="selectDownload"
                @toggle-select="handleToggleSelect"
                @set-bt-speed-limit="handleSetBtSpeedLimit"
                @set-priority="runSetPriority"
              />
            </ErrorBoundary>
          </div>

          <!-- Collapsible bottom detail panel -->
          <ErrorBoundary>
            <DetailPanel
              v-if="selectedId"
              :selected-overview="
                selectedOverview as import('./types/download').DownloadSummary | null
              "
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
    <UiDialog
      v-model="showComposerDialog"
      width="min(46rem, calc(100vw - 1.5rem))"
      :close-on-overlay="false"
    >
      <template #title>
        <div class="dialog-heading dialog-heading--inline">
          <span class="dialog-heading__icon i-ri-download-cloud-2-line" aria-hidden="true" />
          <h2>{{ t("dialog.newTaskTitle") }}</h2>
        </div>
      </template>

      <DownloadComposer :settings="appSettings" @submit="showComposerDialog = false" />
    </UiDialog>

    <BtSpeedLimitModal
      v-model="showBtSpeedLimitModal"
      :task-id="btSpeedLimitTaskId"
      :current-download-limit="btSpeedLimitDownloadLimit"
      :current-upload-limit="btSpeedLimitUploadLimit"
      @confirm="handleBtSpeedLimitConfirm"
    />

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
  transition:
    opacity 250ms ease,
    transform 250ms ease;
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
