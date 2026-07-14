import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

import { getBtRuntimeStatus, getDownloadStatus } from "../lib/tauri/download-api";
import { t } from "../i18n";
import { useNotification } from "./useNotification";
import {
  canPauseState,
  canResumeState,
  terminalStates,
  toFriendlyError,
  toMessage,
  toSummary,
} from "./downloadHelpers";
import { useDownloadActions } from "./useDownloadActions";
import { useDownloadForm } from "./useDownloadForm";
import { useDownloadList } from "./useDownloadList";
import type { BtRuntimeStatus, DownloadProgress, DownloadSnapshot, DownloadSummary } from "../types/download";

export interface UseDownloaderOptions {
  /** Called when a download transitions to failed (for in-app notification) */
  onDownloadFailed?: (fileName: string, reason: string) => void;
  /** Called when one or more downloads are removed from the list */
  onDownloadsRemoved?: (removedIds: string[]) => void;
}

async function fireNotification(title: string, body: string) {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const permission = await requestPermission();
      granted = permission === "granted";
    }
    if (granted) {
      sendNotification({ title, body });
    }
  } catch {
    // Silently fail — notifications are non-critical
  }
}

function createDownloader(options?: UseDownloaderOptions) {
  const downloads = ref<DownloadSummary[]>([]);
  const selectedId = ref<string | null>(null);
  const selectedSnapshot = ref<DownloadSnapshot | null>(null);
  const isAutoRefreshing = ref(false);
  const btRuntimeStatus = ref<BtRuntimeStatus | null>(null);
  const isRefreshingStatus = ref(false);
  const allowAutoSelect = ref(true);
  const actionName = ref("");
  const isStarting = ref(false);
  const notificationsEnabled = ref(false);

  const { notifyInfo, notifyError, clearAll } = useNotification();

  function setMessage(message: string) {
    notifyInfo(message);
  }

  function setError(message: string) {
    notifyError(message);
  }

  function clearMessage() {
    clearAll();
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

  /**
   * Apply a lightweight DownloadProgress patch to an existing download in the list.
   * Mutates fields in-place on the existing reactive object — does NOT create a new array,
   * avoiding full-list recomputation for every progress event.
   */
  function patchProgress(progress: DownloadProgress) {
    const existing = downloads.value.find((d) => d.id === progress.id);
    if (!existing) return;

    // Mutate in-place — Vue 3 reactivity tracks per-field changes
    existing.state = progress.state;
    existing.downloadedBytes = progress.downloadedBytes;
    if (progress.totalBytes !== undefined) existing.totalBytes = progress.totalBytes;
    if (progress.speedBytesPerSecond !== undefined) existing.speedBytesPerSecond = progress.speedBytesPerSecond;
    if (progress.etaSeconds !== undefined) existing.etaSeconds = progress.etaSeconds;
    existing.connectionCount = progress.connectionCount;
    if (progress.allocatedThreadCount !== undefined) existing.allocatedThreadCount = progress.allocatedThreadCount;
    if (progress.error !== undefined) existing.error = progress.error;
    if (progress.uploadedBytes !== undefined) existing.uploadedBytes = progress.uploadedBytes;
    if (progress.uploadSpeedBytesPerSecond !== undefined) existing.uploadSpeedBytesPerSecond = progress.uploadSpeedBytesPerSecond;
    if (progress.peerCount !== undefined) existing.peerCount = progress.peerCount;
    if (progress.uploadStatus !== undefined) existing.uploadStatus = progress.uploadStatus;

    // Patch selectedSnapshot inline (same pattern as existing handleDownloadUpdated patching)
    if (selectedId.value === progress.id && selectedSnapshot.value) {
      Object.assign(selectedSnapshot.value, {
        downloadedBytes: progress.downloadedBytes,
        state: progress.state,
        ...(progress.totalBytes !== undefined && { totalBytes: progress.totalBytes }),
        ...(progress.speedBytesPerSecond !== undefined && { speedBytesPerSecond: progress.speedBytesPerSecond }),
        ...(progress.etaSeconds !== undefined && { etaSeconds: progress.etaSeconds }),
        ...(progress.connectionCount !== undefined && { connectionCount: progress.connectionCount }),
        ...(progress.error !== undefined && { error: progress.error }),
        ...(progress.uploadedBytes !== undefined && { uploadedBytes: progress.uploadedBytes }),
        ...(progress.uploadSpeedBytesPerSecond !== undefined && { uploadSpeedBytesPerSecond: progress.uploadSpeedBytesPerSecond }),
        ...(progress.peerCount !== undefined && { peerCount: progress.peerCount }),
        ...(progress.uploadStatus !== undefined && { uploadStatus: progress.uploadStatus }),
      });
    }
  }

  function removeSummary(downloadId: string) {
    downloads.value = downloads.value.filter((download) => download.id !== downloadId);

    options?.onDownloadsRemoved?.([downloadId]);

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

  async function refreshBtRuntimeStatus(opts?: { silent?: boolean }) {
    try {
      btRuntimeStatus.value = await getBtRuntimeStatus();
    } catch (error) {
      if (!opts?.silent) {
        setError(toMessage(error));
      }
    }
  }

  async function refreshStatus(downloadId = selectedId.value, opts?: { silent?: boolean }) {
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

      if (!opts?.silent) {
        setMessage(t("messages.statusRefreshed", { fileName: snapshot.fileName }));
      }
    } catch (error) {
      if (!opts?.silent) {
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
    onDownloadsRemoved: options?.onDownloadsRemoved,
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
  let unlistenProgress: UnlistenFn | null = null;
  let mounted = true;
  let btRuntimeTimer: ReturnType<typeof setInterval> | null = null;

  function handleDownloadUpdated(summary: DownloadSummary) {
    const existing = downloads.value.find((d) => d.id === summary.id);
    const oldState = existing?.state;

    // In-app notification on state transition to failed
    if (oldState && oldState !== "failed" && summary.state === "failed") {
      options?.onDownloadFailed?.(
        summary.fileName,
        summary.error ? toFriendlyError(summary.error) : t("common.unknown"),
      );
    }

    upsertSummary(summary);

    if (selectedId.value === summary.id && selectedSnapshot.value) {
      // Patch the snapshot with live data from the event, including chunks for heatmap
      selectedSnapshot.value = {
        ...selectedSnapshot.value,
        downloadedBytes: summary.downloadedBytes,
        totalBytes: summary.totalBytes,
        state: summary.state,
        speedBytesPerSecond: summary.speedBytesPerSecond,
        etaSeconds: summary.etaSeconds,
        connectionCount: summary.connectionCount,
        error: summary.error,
        chunks: summary.chunks,
      };
    }

    // Fire OS notifications on genuine state transitions (not initial load)
    if (notificationsEnabled.value && oldState && oldState !== summary.state) {
      if (summary.state === "completed") {
        void fireNotification(t("notifications.downloadComplete"), summary.fileName);
      } else if (summary.state === "failed") {
        void fireNotification(
          t("notifications.downloadFailed"),
          `${summary.fileName}: ${summary.error ? toFriendlyError(summary.error) : ""}`,
        );
      }
    }
  }

  function startAutoRefresh() {
    mounted = true;

    void listen<DownloadProgress>("download-progress", (event) => {
      patchProgress(event.payload);
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenProgress = unlisten;
    });

    void listen<DownloadSummary>("download-updated", (event) => {
      handleDownloadUpdated(event.payload);
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenEvent = unlisten;
    });

    // BT runtime status (global DHT state, upload stats) is not included in per-download
    // events, so a slow background poll is kept.
    btRuntimeTimer = setInterval(() => {
      void refreshBtRuntimeStatus({ silent: true });
    }, 10_000);
  }

  function stopAutoRefresh() {
    mounted = false;

    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }

    if (unlistenEvent) {
      unlistenEvent();
      unlistenEvent = null;
    }

    if (btRuntimeTimer) {
      clearInterval(btRuntimeTimer);
      btRuntimeTimer = null;
    }

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

  function setNotificationsEnabled(enabled: boolean) {
    notificationsEnabled.value = enabled;
  }

  return {
    actionName: actions.actionName,
    setNotificationsEnabled,
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
    isPickingTorrent: form.isPickingTorrent,
    isRefreshingList: list.isRefreshingList,
    isRefreshingStatus,
    isStarting,
    applySchedulerDefaults: form.applySchedulerDefaults,
    applyAppSettingsDefaults: form.applyAppSettingsDefaults,
    pickDestinationDirectory: form.pickDestinationDirectory,
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
    runPauseAll: actions.runPauseAll,
    runResumeAll: actions.runResumeAll,
    runClearCompleted: actions.runClearCompleted,
    runBatchDelete: actions.runBatchDelete,
    selectDownload: actions.selectDownload,
    selectedDownload,
    selectedId: actions.selectedId,
    selectedSnapshot: actions.selectedSnapshot,
    selectedSummary: actions.selectedSummary,
    submitStart: form.submitStart,
  };
}

// Singleton guard — ensures all callers share the same reactive instance.
// useDownloader manages Tauri event listeners and global download state;
// accidental re-instantiation would create duplicate listeners and desync state.
let downloaderInstance: ReturnType<typeof createDownloader> | null = null;

export function useDownloader(options?: UseDownloaderOptions) {
  if (downloaderInstance) {
    if (import.meta.env.DEV && options) {
      console.warn("[useDownloader] Already created — options from this caller ignored.");
    }
    return downloaderInstance;
  }
  downloaderInstance = createDownloader(options);
  return downloaderInstance;
}
