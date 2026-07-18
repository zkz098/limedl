<script setup lang="ts">
import { computed, onErrorCaptured, onMounted, ref, useTemplateRef, watch, type Ref } from "vue";
import { filterDownloads } from "./lib/download-filter";

import CategorySidebar from "./components/layout/CategorySidebar.vue";
import DownloadComposer from "./components/flareget/DownloadComposer.vue";
import DownloadQueueTable from "./components/flareget/DownloadQueueTable.vue";
import DetailPanel from "./components/flareget/DetailPanel.vue";
import LabsPage from "./components/labs/LabsPage.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import TopToolbar from "./components/layout/TopToolbar.vue";
import UiButton from "./components/ui/UiButton.vue";
import ConfirmDialog from "./components/ui/ConfirmDialog.vue";
import UiDialog from "./components/ui/UiDialog.vue";
import { useFlareget } from "./composables/useFlareget";
import type { UseFlaregetOptions } from "./composables/useFlareget";
import { useIoBaseline } from "./composables/useIoBaseline";
import { useOverclock } from "./composables/useOverclock";
import { useCategoryCounts } from "./composables/useCategoryCounts";
import { useNotification } from "./composables/useNotification";
import { useI18n } from "./i18n";
import { useAppSettings } from "./composables/useAppSettings";
import { useViewNavigation } from "./composables/useViewNavigation";
import { useMultiSelect } from "./composables/useMultiSelect";
import { useAppUpdate } from "./composables/useAppUpdate";
import { DEFAULT_VISIBLE_COLUMNS } from "./lib/column-defs";
import NotificationToast from "./components/ui/NotificationToast.vue";
import ModalOverlay from "./components/layout/ModalOverlay.vue";
import type { AppSettings, SortDirection, SortKey } from "./types/settings";
import type { ViewOptions, MultiSelectState } from "./types/download";

// Multi-select refs (declared before flaregetOptions closure)
let multiSelectMode = ref(false);
let selectedIds = ref<Set<string>>(new Set());
let showBatchDeleteDialog = ref(false);
let removedDownloadIds = ref<string[]>([]);

const flaregetOptions: UseFlaregetOptions = {
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
} = useFlareget(flaregetOptions);

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
const { gameMode, bufferUsageBytes, bufferLimitBytes, activeSlots, maxSlots, queuedCount, setGameMode } = useIoBaseline();
const { overclockMode, setOverclockMode } = useOverclock();
const showComposerDialog = ref(false);
const activeCategory = ref("");
const searchQuery = ref("");
const sortKey = ref<SortKey>("added_at");
const sortDirection = ref<SortDirection>("desc");
const compactView = ref(false);
const visibleColumns = ref<string[]>([...DEFAULT_VISIBLE_COLUMNS]);
const pendingPermanentDeleteId = ref<string | null>(null);
const settingsPageRef = useTemplateRef<InstanceType<typeof SettingsPage>>("settingsPage");
const labsPageRef = useTemplateRef<InstanceType<typeof LabsPage>>("labsPage");

const { appSettings, applyAppearanceSettings } = useAppSettings({
  sortKey,
  sortDirection,
  compactView,
  visibleColumns,
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
  <div class="app-root">
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
          </div>

          <!-- Collapsible bottom detail panel -->
          <DetailPanel
            v-if="selectedId"
            :selected-overview="selectedOverview"
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
        </template>
      </main>
    </div>

    <!-- Settings & Labs as centered modal overlays -->
    <ModalOverlay :model-value="currentView === 'settings'" @close="navigateTo('home')">
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
      />
    </ModalOverlay>
    <ModalOverlay :model-value="currentView === 'labs'" @close="navigateTo('home')">
      <LabsPage
        ref="labsPage"
        :settings="appSettings"
        @dirty-change="handleLabsDirtyChange"
        @saved="handleLabsSaved"
      />
    </ModalOverlay>

    <!-- Dialogs -->
    <UiDialog v-model="showComposerDialog" width="min(46rem, calc(100vw - 1.5rem))">
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
</style>
