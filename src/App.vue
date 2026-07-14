<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from "vue";
import { filterDownloads } from "./lib/download-filter";
import { debounce } from "es-toolkit";

import CategorySidebar from "./components/layout/CategorySidebar.vue";
import DownloadComposer from "./components/downloader/DownloadComposer.vue";
import DownloadInspector from "./components/downloader/DownloadInspector.vue";
import DownloadQueueTable from "./components/downloader/DownloadQueueTable.vue";
import LabsPage from "./components/labs/LabsPage.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import TopToolbar from "./components/layout/TopToolbar.vue";
import UiBadge from "./components/ui/UiBadge.vue";
import UiButton from "./components/ui/UiButton.vue";
import UiDialog from "./components/ui/UiDialog.vue";
import { getAppSettings, saveAppSettings } from "./lib/tauri/settings-api";
import { VALID_COLUMN_KEY_SET, DEFAULT_VISIBLE_COLUMNS } from "./lib/column-defs";
import { useDownloader } from "./composables/useDownloader";
import type { UseDownloaderOptions } from "./composables/useDownloader";
import { useIoBaseline } from "./composables/useIoBaseline";
import { useNotification } from "./composables/useNotification";
import { useI18n } from "./i18n";
import NotificationToast from "./components/ui/NotificationToast.vue";
import type { AppSettings, ColorMode, SortDirection, SortKey } from "./types/settings";
import type { ViewOptions, MultiSelectState } from "./components/downloader/queue-types";

const downloaderOptions: UseDownloaderOptions = {
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
  setNotificationsEnabled,
} = useDownloader(downloaderOptions);

const { t } = useI18n();
const { gameMode, bufferUsageBytes, bufferLimitBytes, setGameMode } = useIoBaseline();
const showComposerDialog = ref(false);
const detailCollapsed = ref(false);
const currentView = ref<"home" | "settings" | "labs">("home");
const appSettings = ref<AppSettings | null>(null);
const activeCategory = ref("");
const searchQuery = ref("");
const sortKey = ref<SortKey>("added_at");
const sortDirection = ref<SortDirection>("desc");
const compactView = ref(false);
const visibleColumns = ref<string[]>([...DEFAULT_VISIBLE_COLUMNS]);
const isSyncingFromSettings = ref(false);
const pendingPermanentDeleteId = ref<string | null>(null);
const pendingView = ref<"home" | "settings" | "labs" | null>(null);
const settingsHasUnsavedChanges = ref(false);
const labsHasUnsavedChanges = ref(false);
const isSavingBeforeNavigation = ref(false);
const settingsPageRef = useTemplateRef<InstanceType<typeof SettingsPage>>("settingsPage");
const labsPageRef = useTemplateRef<InstanceType<typeof LabsPage>>("labsPage");
let colorSchemeQuery: MediaQueryList | null = null;

// Multi-select mode
const multiSelectMode = ref(false);
const selectedIds = ref<Set<string>>(new Set());
const showBatchDeleteDialog = ref(false);
const removedDownloadIds = ref<string[]>([]);

const filteredDownloads = computed(() =>
  filterDownloads(downloads.value, searchQuery.value, activeCategory.value),
);

const { notifications, notifyError, dismiss } = useNotification();

function handleSystemColorSchemeChange() {
  applyColorMode(appSettings.value?.appearance?.colorMode ?? "system");
}

const selectedOverview = computed(() => selectedSnapshot.value ?? selectedSummary.value);

const stateTone = computed<"neutral" | "info" | "success" | "warning" | "danger">(() => {
  const state = selectedOverview.value?.state;

  if (!state) return "neutral";
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
});

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

const categoryCounts = computed(() => {
  const all = downloads.value.length;
  const downloading = downloads.value.filter((d) => d.state === "downloading").length;
  const paused = downloads.value.filter((d) => d.state === "paused").length;
  const completed = downloads.value.filter((d) => d.state === "completed").length;
  const failed = downloads.value.filter((d) => d.state === "failed").length;
  const active = downloads.value.filter((d) => d.state === "downloading").length;
  return { "": all, downloading, paused, completed, failed, active };
});

const sidebarStats = computed(() => {
  const totalSpeed = downloads.value.reduce((sum, d) => sum + (d.speedBytesPerSecond ?? 0), 0);
  return {
    totalTasks: downloads.value.length,
    activeTasks: downloads.value.filter((d) => d.state === "downloading").length,
    completedTasks: downloads.value.filter((d) => d.state === "completed").length,
    currentSpeed: totalSpeed,
  };
});

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

// Multi-select handlers
function handleToggleMultiSelectMode(enabled: boolean) {
  multiSelectMode.value = enabled;
  if (!enabled) {
    selectedIds.value = new Set();
  }
}

function handleToggleSelect(downloadId: string) {
  const next = new Set(selectedIds.value);
  if (next.has(downloadId)) {
    next.delete(downloadId);
  } else {
    next.add(downloadId);
  }
  selectedIds.value = next;
}

function handleSelectAll() {
  selectedIds.value = new Set(filteredDownloads.value.map((d) => d.id));
}

function handleDeselectAll() {
  selectedIds.value = new Set();
}

function handleBatchDelete() {
  showBatchDeleteDialog.value = true;
}

async function handleToggleGameMode() {
  await setGameMode(!gameMode.value);
}

async function confirmBatchDelete() {
  const ids = [...selectedIds.value];
  showBatchDeleteDialog.value = false;
  if (ids.length === 0) return;
  await runBatchDelete(ids);
  selectedIds.value = new Set();
}

function navigateTo(view: string) {
  const validViews = ["home", "settings", "labs"] as const;
  if (!validViews.includes(view as (typeof validViews)[number])) {
    return;
  }

  const typedView = view as "home" | "settings" | "labs";
  if (typedView === currentView.value) {
    return;
  }

  const leavingDirtyView =
    (currentView.value === "settings" && settingsHasUnsavedChanges.value) ||
    (currentView.value === "labs" && labsHasUnsavedChanges.value);

  if (leavingDirtyView) {
    pendingView.value = typedView;
    return;
  }

  currentView.value = typedView;
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
  document.documentElement.dataset.theme = settings.appearance?.themeColor ?? "lime";
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
      detailCollapsed.value = false;
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
  appSettings,
  (settings) => {
    setNotificationsEnabled(settings?.notifications?.enabled ?? false);
  },
  { immediate: true },
);

watch(
  appSettings,
  (settings) => {
    if (!settings) {
      return;
    }

    isSyncingFromSettings.value = true;
    sortKey.value = settings.appearance?.sortKey ?? "added_at";
    sortDirection.value = settings.appearance?.sortDirection ?? "desc";
    compactView.value = settings.appearance?.compactView ?? false;
    const loaded = (settings.appearance?.visibleColumns ?? [...DEFAULT_VISIBLE_COLUMNS]).filter(
      (k) => VALID_COLUMN_KEY_SET.has(k),
    );
    if (!loaded.includes("file")) {
      loaded.unshift("file");
    }
    visibleColumns.value = loaded;
    nextTick(() => {
      isSyncingFromSettings.value = false;
    });
  },
  { immediate: true },
);

const debouncedSaveAppearance = debounce(async () => {
  if (!appSettings.value) return;
  try {
    await saveAppSettings(appSettings.value);
  } catch (error) {
    console.error("Failed to save view settings", error);
  }
}, 300);

watch([sortKey, sortDirection, compactView, visibleColumns], () => {
  if (isSyncingFromSettings.value || !appSettings.value) {
    return;
  }

  appSettings.value.appearance.sortKey = sortKey.value;
  appSettings.value.appearance.sortDirection = sortDirection.value;
  appSettings.value.appearance.compactView = compactView.value;
  appSettings.value.appearance.visibleColumns = [...visibleColumns.value];

  debouncedSaveAppearance();
});

// Watcher #1 (failed detection) and Watcher #2 (stale selectedIds cleanup)
// have been replaced with event-driven callbacks in useDownloader().

onBeforeUnmount(() => {
  colorSchemeQuery?.removeEventListener("change", handleSystemColorSchemeChange);
});
</script>

<template>
  <div class="app-root">
    <NotificationToast :notifications="notifications" @dismiss="dismiss" />

    <!-- Top toolbar (only show on home view) -->
    <TopToolbar v-if="currentView === 'home'" :search-query="searchQuery" :has-selection="!!selectedId"
      :bt-status="btStatusData" :sort-key="sortKey" :sort-direction="sortDirection" :compact-view="compactView"
      :visible-columns="visibleColumns" :multi-select-mode="multiSelectMode" :selected-count="selectedIds.size"
      :filtered-count="filteredDownloads.length" :game-mode="gameMode"
      :game-mode-buffer-mb="appSettings?.ioBaseline?.gameModeBufferMb" @update:search-query="searchQuery = $event"
      @update:sort-key="sortKey = $event" @update:sort-direction="sortDirection = $event"
      @update:compact-view="compactView = $event" @update:visible-columns="visibleColumns = $event"
      @add-task="showComposerDialog = true" @delete="handleDelete" @refresh="handleRefresh"
      @update:multi-select-mode="handleToggleMultiSelectMode" @pause-all="runPauseAll" @resume-all="runResumeAll"
      @clear-completed="runClearCompleted" @select-all="handleSelectAll" @deselect-all="handleDeselectAll"
      @batch-delete="handleBatchDelete" @toggle-game-mode="handleToggleGameMode" />

    <!-- Main layout: sidebar + content -->
    <div class="app-body">
      <CategorySidebar :active-category="activeCategory" :current-view="currentView" :counts="categoryCounts"
        :stats="sidebarStats" @update:active-category="activeCategory = $event" @navigate="navigateTo" />

      <main class="content-area">
        <!-- Home view: table + detail panel -->
        <template v-if="currentView === 'home'">
          <div class="table-wrapper">
            <DownloadQueueTable :downloads="downloads" :selected-id="selectedId" :task-action-name="actionName"
              :is-auto-refreshing="isAutoRefreshing" :state-filter="activeCategory" :search-query="searchQuery"
              :view-options="viewOptions" :multi-select="multiSelectState" @copy-link="runCopyLink"
              @delete-task="runDeleteTask" @delete-task-permanently="requestPermanentDelete"
              @open-in-explorer="runOpenInExplorer" @pause-or-resume="handleTaskPauseOrResume" @select="selectDownload"
              @toggle-select="handleToggleSelect" />
          </div>

          <!-- Collapsible bottom detail panel -->
          <div class="detail-panel" :class="{ collapsed: detailCollapsed }">
            <div class="detail-panel__header" @click="detailCollapsed = !detailCollapsed">
              <div class="detail-panel__title">
                <i class="detail-panel__arrow"
                  :class="detailCollapsed ? 'i-ri-arrow-up-line' : 'i-ri-arrow-down-line'" />
                <span class="detail-panel__filename">{{
                  selectedOverview ? selectedOverview.fileName : t("detail.noSelection")
                  }}</span>
                <template v-if="selectedOverview">
                  <UiBadge :tone="stateTone" size="sm">{{
                    t(`states.${selectedOverview.state}`)
                    }}</UiBadge>
                  <UiBadge v-if="selectedOverview.cdnAccelerated" tone="warning" size="sm" class="detail-panel__cdn">
                    <span class="i-ri-flashlight-fill" aria-hidden="true" />
                    {{ t("inspector.cdnAccelerated") }}
                  </UiBadge>
                </template>
              </div>
              <div v-if="selectedOverview" class="detail-panel__actions">
                <UiButton type="button" size="sm" variant="secondary" icon="i-ri-refresh-line"
                  @click.stop="handleRefreshSelected">
                  {{ isRefreshingStatus ? t("common.refreshing") : t("common.refresh") }}
                </UiButton>
                <UiButton type="button" size="sm" variant="ghost" icon="i-ri-pause-line" :disabled="!canPause"
                  @click.stop="runPause">
                  {{ actionName === "Pause" ? t("inspector.pausing") : t("inspector.pause") }}
                </UiButton>
                <UiButton type="button" size="sm" variant="ghost" icon="i-ri-play-line" :disabled="!canResume"
                  @click.stop="runResume">
                  {{ actionName === "Resume" ? t("inspector.resuming") : t("inspector.resume") }}
                </UiButton>
                <UiButton type="button" size="sm" variant="danger" icon="i-ri-close-circle-line" :disabled="!canCancel"
                  @click.stop="runCancel">
                  {{ actionName === "Cancel" ? t("inspector.canceling") : t("inspector.cancel") }}
                </UiButton>
                <UiButton type="button" size="sm" variant="ghost" icon="i-ri-close-line"
                  @click.stop="selectDownload(null)" />
              </div>
            </div>
            <div v-show="!detailCollapsed" class="detail-panel__body">
              <DownloadInspector v-if="selectedOverview" :selected-overview="selectedOverview"
                :selected-snapshot="selectedSnapshot" :show-detail-info="showDetailInfo" />
              <div v-else class="detail-panel__empty">
                <i class="i-ri-cursor-line" />
                <p>{{ t("detail.selectPrompt") }}</p>
              </div>
            </div>
          </div>
        </template>
      </main>
    </div>

    <!-- Settings & Labs as centered modal overlays -->
    <Transition name="overlay-fade">
      <div v-if="currentView === 'settings'" class="fullscreen-overlay">
        <div class="modal-panel">
          <button class="overlay-close" @click="navigateTo('home')" :title="t('nav.back')">
            <i class="i-ri-close-line" />
          </button>
          <div class="modal-panel__body">
            <SettingsPage ref="settingsPage" :settings="appSettings" :game-mode="gameMode"
              :buffer-usage-bytes="bufferUsageBytes" :buffer-limit-bytes="bufferLimitBytes"
              @dirty-change="handleSettingsDirtyChange" @saved="handleSettingsSaved" />
          </div>
        </div>
      </div>
    </Transition>
    <Transition name="overlay-fade">
      <div v-if="currentView === 'labs'" class="fullscreen-overlay">
        <div class="modal-panel">
          <button class="overlay-close" @click="navigateTo('home')" :title="t('nav.back')">
            <i class="i-ri-close-line" />
          </button>
          <div class="modal-panel__body">
            <LabsPage ref="labsPage" :settings="appSettings" @dirty-change="handleLabsDirtyChange"
              @saved="handleLabsSaved" />
          </div>
        </div>
      </div>
    </Transition>

    <!-- Dialogs -->
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

      <DownloadComposer :form="form" :is-picking-directory="isPickingDirectory" :is-picking-torrent="isPickingTorrent"
        :is-starting="isStarting" :settings="appSettings" @pick-directory="pickDestinationDirectory"
        @pick-torrent="pickTorrentSourceFile" @submit="handleSubmitStart" />
    </UiDialog>

    <UiDialog :model-value="Boolean(pendingPermanentDeleteId)" width="min(32rem, calc(100vw - 1.5rem))"
      :close-on-overlay="actionName !== 'Purge'" @update:model-value="
        (value) => {
          if (!value) cancelPermanentDelete();
        }
      ">
      <template #title>
        <div class="dialog-heading">
          <div>
            <p class="section-kicker">{{ t("dialog.confirmDelete") }}</p>
            <h2>{{ t("dialog.permanentDeleteTitle") }}</h2>
          </div>
          <span class="dialog-heading__icon dialog-heading__icon--danger i-ri-delete-bin-line" aria-hidden="true" />
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
          <UiButton type="button" variant="secondary" :disabled="actionName === 'Purge'" @click="cancelPermanentDelete">
            {{ t("common.cancel") }}
          </UiButton>
          <UiButton type="button" variant="danger" icon="i-ri-delete-bin-line" :loading="actionName === 'Purge'"
            @click="confirmPermanentDelete">
            {{ actionName === "Purge" ? t("dialog.deleting") : t("dialog.confirmPermanentDelete") }}
          </UiButton>
        </div>
      </div>
    </UiDialog>

    <UiDialog :model-value="showUnsavedSettingsDialog" width="min(32rem, calc(100vw - 1.5rem))" @update:model-value="
      (value) => {
        if (!value) cancelDiscardSettings();
      }
    ">
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
          <UiButton type="button" icon="i-ri-save-line" :loading="isSavingBeforeNavigation"
            @click="saveSettingsAndNavigate">
            {{ isSavingBeforeNavigation ? t("common.saving") : t("dialog.saveSettingsAndLeave") }}
          </UiButton>
          <UiButton type="button" variant="danger" icon="i-ri-arrow-right-line" @click="confirmDiscardSettings">
            {{ t("dialog.discardSettings") }}
          </UiButton>
        </div>
      </div>
    </UiDialog>

    <!-- Batch delete dialog -->
    <UiDialog :model-value="showBatchDeleteDialog" width="min(32rem, calc(100vw - 1.5rem))" @update:model-value="
      (value) => {
        if (!value) showBatchDeleteDialog = false;
      }
    ">
      <template #title>
        <div class="dialog-heading">
          <div>
            <p class="section-kicker">{{ t("dialog.confirmDelete") }}</p>
            <h2>{{ t("dialog.batchDeleteTitle") }}</h2>
          </div>
        </div>
      </template>

      <div class="confirm-delete">
        <p class="confirm-delete__message">
          {{ t("dialog.batchDeleteMessage", { count: selectedIds.size }) }}
        </p>
        <div class="confirm-delete__actions">
          <UiButton type="button" variant="secondary" @click="showBatchDeleteDialog = false">
            {{ t("common.cancel") }}
          </UiButton>
          <UiButton type="button" variant="danger" icon="i-ri-delete-bin-line" @click="confirmBatchDelete">
            {{ t("dialog.confirmBatchDelete") }}
          </UiButton>
        </div>
      </div>
    </UiDialog>
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

/* ── Detail panel ── */

.detail-panel {
  flex-shrink: 0;
  border-top: 1px solid var(--color-border);
  background: var(--color-panel);
  max-height: 40vh;
  display: flex;
  flex-direction: column;
  transition: max-height 0.2s ease;
}

.detail-panel.collapsed {
  max-height: 3.75rem;
}

.detail-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  cursor: pointer;
  user-select: none;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.detail-panel.collapsed .detail-panel__header {
  border-bottom: none;
}

.detail-panel__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  min-width: 0;
  font-size: 0.875rem;
  font-weight: 500;
  color: var(--color-text-main);
}

.detail-panel__arrow {
  color: var(--color-accent);
  flex-shrink: 0;
}

.detail-panel__filename {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.detail-panel__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  flex-wrap: wrap;
}

.detail-panel__cdn {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
}

.detail-panel__cdn .i-ri-flashlight-fill {
  font-size: 0.8rem;
}

.detail-panel__body {
  flex: 1;
  overflow: auto;
  min-height: 0;
}

.detail-panel__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  color: var(--color-text-soft);
  gap: var(--space-2);
}

.detail-panel__empty i {
  font-size: 1.5rem;
}

/* ── Modal overlays ── */

.fullscreen-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  overflow: auto;
  background: var(--surface-overlay-bg);
  -webkit-backdrop-filter: blur(8px);
  backdrop-filter: blur(8px);
}

.modal-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 64rem;
  height: calc(100vh - 2 * var(--space-6));
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  box-shadow:
    0 8px 32px oklch(0 0 0 / 0.12),
    0 2px 8px oklch(0 0 0 / 0.08);
  overflow: hidden;
}

.modal-panel__body {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--space-4) var(--space-4) 0;
}

.overlay-close {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  z-index: 10;
  width: 2.25rem;
  height: 2.25rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-panel);
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 1.125rem;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.overlay-close:hover {
  background: var(--color-surface-muted);
  color: var(--color-text-main);
}

.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 0.2s ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
}

.overlay-fade-enter-active .modal-panel,
.overlay-fade-leave-active .modal-panel {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}

.overlay-fade-enter-from .modal-panel,
.overlay-fade-leave-to .modal-panel {
  transform: scale(0.97);
  opacity: 0;
}

@media (max-width: 680px) {
  .fullscreen-overlay {
    padding: var(--space-4);
  }

  .modal-panel {
    max-height: calc(100vh - 2 * var(--space-4));
  }

  .modal-panel__body {
    padding: var(--space-3) var(--space-3) 0;
  }
}

/* ── Dialog styles ── */

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

.dialog-heading__icon {
  font-size: 1.25rem;
  color: var(--color-text-muted);
}

.dialog-heading__icon--danger {
  color: var(--color-danger-text);
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
</style>
