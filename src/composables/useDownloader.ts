import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { getBtRuntimeStatus, getDownloadStatus } from "../lib/tauri/download-api";
import { t } from "../i18n";
import { useNotification } from "./useNotification";
import {
  canPauseState,
  canResumeState,
  terminalStates,
  toMessage,
  toSummary,
} from "./downloadHelpers";
import { useDownloadActions } from "./useDownloadActions";
import { useDownloadForm } from "./useDownloadForm";
import { useDownloadList } from "./useDownloadList";
import type { BtRuntimeStatus, DownloadSnapshot, DownloadSummary } from "../types/download";

function clearMessage() { }

export function useDownloader() {
  const downloads = ref<DownloadSummary[]>([]);
  const selectedId = ref<string | null>(null);
  const selectedSnapshot = ref<DownloadSnapshot | null>(null);
  const isAutoRefreshing = ref(false);
  const btRuntimeStatus = ref<BtRuntimeStatus | null>(null);
  const isRefreshingStatus = ref(false);
  const allowAutoSelect = ref(true);
  const actionName = ref("");
  const isStarting = ref(false);

  const { notifyInfo, notifyError } = useNotification();

  function setMessage(message: string) {
    notifyInfo(message);
  }

  function setError(message: string) {
    notifyError(message);
  }

  function upsertSummary(summary: DownloadSummary) {
    const next = [...downloads.value];
    const index = next.findIndex((download) => download.id === summary.id);

    if (index >= 0) {
      next[index] = summary;
    } else {
      next.unshift(summary);
    }

    downloads.value = next;
  }

  function removeSummary(downloadId: string) {
    downloads.value = downloads.value.filter((download) => download.id !== downloadId);

    if (selectedId.value === downloadId) {
      allowAutoSelect.value = false;
      selectedId.value = null;
      selectedSnapshot.value = null;
    }
  }

  function ensureSelection() {
    if (selectedId.value && downloads.value.some((download) => download.id === selectedId.value)) {
      return;
    }

    if (!allowAutoSelect.value) {
      selectedId.value = null;
      selectedSnapshot.value = null;
      return;
    }

    selectedId.value = downloads.value[0]?.id ?? null;

    if (!selectedId.value) {
      selectedSnapshot.value = null;
    }
  }

  const selectedSummary = computed(() => {
    if (!selectedId.value) {
      return null;
    }

    return downloads.value.find((download) => download.id === selectedId.value) ?? null;
  });

  const selectedDownload = computed(() => selectedSnapshot.value ?? selectedSummary.value);

  const canPause = computed(() => {
    return canPauseState(selectedDownload.value?.state);
  });

  const canResume = computed(() => canResumeState(selectedDownload.value?.state));

  const canCancel = computed(() => {
    const state = selectedDownload.value?.state;

    if (!state) {
      return false;
    }

    return !terminalStates.includes(state);
  });

  function shouldRefreshSelectedStatus() {
    const state = selectedSnapshot.value?.state ?? selectedSummary.value?.state;

    if (!selectedId.value) {
      return false;
    }

    if (!state) {
      return true;
    }

    return !terminalStates.includes(state);
  }

  async function refreshBtRuntimeStatus(options?: { silent?: boolean }) {
    try {
      btRuntimeStatus.value = await getBtRuntimeStatus();
    } catch (error) {
      if (!options?.silent) {
        setError(toMessage(error));
      }
    }
  }

  async function refreshStatus(downloadId = selectedId.value, options?: { silent?: boolean }) {
    if (!downloadId) {
      return;
    }

    if (isRefreshingStatus.value) {
      return;
    }

    isRefreshingStatus.value = true;

    try {
      const snapshot = await getDownloadStatus(downloadId);
      upsertSummary(toSummary(snapshot));

      if (selectedId.value === downloadId) {
        selectedSnapshot.value = snapshot;
      }

      if (!options?.silent) {
        setMessage(t("messages.statusRefreshed", { fileName: snapshot.fileName }));
      }
    } catch (error) {
      if (!options?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingStatus.value = false;
    }
  }

  const list = useDownloadList({
    downloads,
    selectedId,
    selectedSnapshot,
    allowAutoSelect,
    isAutoRefreshing,
    ensureSelection,
    setMessage,
    setError,
  });

  const actions = useDownloadActions({
    downloads,
    selectedId,
    selectedSnapshot,
    actionName,
    allowAutoSelect,
    selectedSummary,
    selectedDownload,
    canPause,
    canResume,
    canCancel,
    shouldRefreshSelectedStatus,
    upsertSummary,
    removeSummary,
    refreshStatus,
    setMessage,
    setError,
    clearMessage,
  });

  const form = useDownloadForm({
    selectedId,
    allowAutoSelect,
    isStarting,
    upsertSummary,
    refreshList: list.refreshList,
    refreshStatus,
    setMessage,
    setError,
    clearMessage,
  });

  let unlistenEvent: UnlistenFn | null = null;
  let fallbackTimer: ReturnType<typeof setInterval> | null = null;
  let autoRefreshInFlight = false;
  let refreshChunksTimer: ReturnType<typeof setTimeout> | null = null;

  function handleDownloadUpdated(summary: DownloadSummary) {
    upsertSummary(summary);

    if (selectedId.value === summary.id && selectedSnapshot.value) {
      selectedSnapshot.value = {
        ...selectedSnapshot.value,
        downloadedBytes: summary.downloadedBytes,
        totalBytes: summary.totalBytes,
        state: summary.state,
        speedBytesPerSecond: summary.speedBytesPerSecond,
        etaSeconds: summary.etaSeconds,
        connectionCount: summary.connectionCount,
        error: summary.error,
      };

      // Debounced full snapshot refresh so chunk progress (heatmap) stays current
      if (refreshChunksTimer) {
        clearTimeout(refreshChunksTimer);
      }
      refreshChunksTimer = setTimeout(() => {
        if (selectedId.value === summary.id && !terminalStates.includes(summary.state)) {
          void refreshStatus(summary.id, { silent: true });
        }
      }, 2_000);
    }
  }

  async function runAutoRefresh() {
    if (autoRefreshInFlight || isStarting.value || Boolean(actionName.value)) {
      return;
    }

    autoRefreshInFlight = true;
    isAutoRefreshing.value = true;

    try {
      await refreshBtRuntimeStatus({ silent: true });

      if (shouldRefreshSelectedStatus()) {
        await refreshStatus(selectedId.value, { silent: true });
      }
    } finally {
      isAutoRefreshing.value = false;
      autoRefreshInFlight = false;
    }
  }

  function startAutoRefresh() {
    void listen<DownloadSummary>("download-updated", (event) => {
      handleDownloadUpdated(event.payload);
    }).then((unlisten) => {
      unlistenEvent = unlisten;
    });

    fallbackTimer = setInterval(() => {
      void runAutoRefresh();
    }, 3_000);
  }

  function stopAutoRefresh() {
    if (unlistenEvent) {
      unlistenEvent();
      unlistenEvent = null;
    }

    if (fallbackTimer) {
      clearInterval(fallbackTimer);
      fallbackTimer = null;
    }

    if (refreshChunksTimer) {
      clearTimeout(refreshChunksTimer);
      refreshChunksTimer = null;
    }

    autoRefreshInFlight = false;
    isAutoRefreshing.value = false;
  }

  onMounted(async () => {
    await list.refreshList({ silent: true });
    await refreshBtRuntimeStatus({ silent: true });

    if (selectedId.value) {
      await refreshStatus(selectedId.value, { silent: true });
    }

    startAutoRefresh();
  });

  onUnmounted(() => {
    stopAutoRefresh();
  });

  return {
    actionName: actions.actionName,
    canCancel,
    canPause,
    canResume,
    canPauseDownload: actions.canPauseDownload,
    canResumeDownload: actions.canResumeDownload,
    btRuntimeStatus,
    downloads: list.downloads,
    form: form.form,
    isAutoRefreshing: list.isAutoRefreshing,
    isPickingDirectory: form.isPickingDirectory,
    isPickingMetalink: form.isPickingMetalink,
    isPickingTorrent: form.isPickingTorrent,
    isRefreshingList: list.isRefreshingList,
    isRefreshingStatus,
    isStarting,
    applySchedulerDefaults: form.applySchedulerDefaults,
    applyAppSettingsDefaults: form.applyAppSettingsDefaults,
    pickDestinationDirectory: form.pickDestinationDirectory,
    pickMetalinkSourceFile: form.pickMetalinkSourceFile,
    pickTorrentSourceFile: form.pickTorrentSourceFile,
    refreshList: list.refreshList,
    refreshBtRuntimeStatus,
    refreshStatus,
    runCancel: actions.runCancel,
    runDeleteTask: actions.runDeleteTask,
    runDeleteTaskPermanently: actions.runDeleteTaskPermanently,
    runCopyLink: actions.runCopyLink,
    runOpenInExplorer: actions.runOpenInExplorer,
    runPause: actions.runPause,
    runPauseFor: actions.runPauseFor,
    runResume: actions.runResume,
    runResumeFor: actions.runResumeFor,
    selectDownload: actions.selectDownload,
    selectedDownload,
    selectedId: actions.selectedId,
    selectedSnapshot: actions.selectedSnapshot,
    selectedSummary: actions.selectedSummary,
    submitStart: form.submitStart,
  };
}
