import { storeToRefs } from "pinia";
import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "#event";
import {
  openDownloadInExplorer,
  setBtSpeedLimit,
  startDownload,
} from "../../lib/tauri/download-api";
import { t } from "../../i18n";
import { toFriendlyError, toMessage } from "../../composables/downloadHelpers";
import { useNotificationStore } from "../notification";
import type { DownloadProgress, DownloadSummary } from "../../types/download";
import type { DownloadStoreOptions } from "./types";
import { fireNotification } from "./helpers";
import { useDownloadListStore } from "./list";
import { useDownloadFormStore } from "./form";
import { useBtDetailStore } from "./bt";
import { onAction } from "../../lib/platform";

export * from "./types";
export * from "./helpers";
export * from "./list";
export * from "./form";
export * from "./bt";

export const useDownloadStore = defineStore("download", () => {
  const notify = useNotificationStore();
  const listStore = useDownloadListStore();
  const formStore = useDownloadFormStore();
  const btStore = useBtDetailStore();

  const {
    downloads,
    selectedId,
    selectedSnapshot,
    selectedSummary,
    selectedDownload,
    isAutoRefreshing,
    actionName,
    notificationsEnabled,
    isRefreshingList,
    canPause,
    canResume,
    canCancel,
  } = storeToRefs(listStore);

  const {
    form,
    isPickingDirectory,
    isPickingTorrent,
    isStarting,
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
  } = storeToRefs(formStore);

  const { btRuntimeStatus, isRefreshingStatus } = storeToRefs(btStore);

  function configure(opts: DownloadStoreOptions) {
    listStore.configure(opts);
  }

  function setMessage(message: string) {
    listStore.setMessage(message);
  }

  function setError(message: string) {
    listStore.setError(message);
  }

  function setNotificationsEnabled(enabled: boolean) {
    listStore.setNotificationsEnabled(enabled);
  }

  async function refreshStatus(downloadId = selectedId.value, opts?: { silent?: boolean }) {
    await btStore.refreshStatus(
      downloadId,
      (snapshot, summary) => {
        listStore.upsertSummary(summary);
        if (selectedId.value === downloadId) {
          selectedSnapshot.value = snapshot;
        }
      },
      opts,
    );
  }

  async function selectDownload(downloadId: string | null) {
    await listStore.selectDownload(downloadId, async (id, opts) => {
      await refreshStatus(id, opts);
    });
  }

  async function submitStart() {
    if (isStarting.value) return;

    if (!form.value.url.trim() || !form.value.destinationDir.trim()) {
      listStore.setError(
        form.value.kind === "bt" ? t("messages.torrentStartRequired") : t("messages.startRequired"),
      );
      return;
    }

    isStarting.value = true;

    try {
      listStore.clearMessage();

      const taskId = await startDownload(formStore.buildStartRequest());
      const rawId = typeof taskId === "string" ? taskId : (taskId?.id ?? "mock-id");

      listStore.allowAutoSelect = true;
      selectedId.value = rawId;

      if (form.value.kind === "bt") {
        const dl = form.value.downloadLimitBps;
        const ul = form.value.uploadLimitBps;
        if ((dl !== null && dl > 0) || (ul !== null && ul > 0)) {
          try {
            await setBtSpeedLimit(rawId, dl ?? undefined, ul ?? undefined);
          } catch {
            // Non-critical
          }
        }
      }

      await listStore.refreshList();
      await refreshStatus(rawId, { silent: true });
      listStore.setMessage(t("messages.downloadQueued", { id: rawId }));
      formStore.resetForm();
    } catch (error) {
      listStore.setError(toMessage(error));
    } finally {
      isStarting.value = false;
    }
  }

  async function submitBatch(): Promise<void> {
    if (isStarting.value) return;
    if (!form.value.destinationDir.trim()) {
      listStore.setError(t("messages.startRequired"));
      return;
    }
    if (batchEntries.value.length === 0) {
      listStore.setError(t("composer.batchEmpty"));
      return;
    }

    isStarting.value = true;
    batchSubmitProgress.value = { done: 0, total: batchEntries.value.length };
    listStore.clearMessage();

    try {
      let successCount = 0;
      const errors: string[] = [];
      let completedCount = 0;

      const results = await Promise.all(
        batchEntries.value.map(async (entry) => {
          try {
            entry.status = "queued";
            await startDownload(formStore.buildBatchRequest(entry));
            entry.status = "success";
            return true;
          } catch (error) {
            entry.status = "error";
            entry.error = toMessage(error);
            errors.push(`${entry.fileName || entry.url}: ${toMessage(error)}`);
            return false;
          } finally {
            completedCount++;
            batchSubmitProgress.value = { done: completedCount, total: batchEntries.value.length };
          }
        }),
      );
      successCount = results.filter(Boolean).length;

      await listStore.refreshList();
      if (errors.length > 0) {
        listStore.setError(
          t("composer.batchCompletedWithErrors", {
            success: successCount,
            total: batchEntries.value.length,
            errors: errors.join("; "),
          }),
        );
      } else {
        listStore.setMessage(
          t("composer.batchCompleted", { success: successCount, total: batchEntries.value.length }),
        );
      }

      batchEntries.value = batchEntries.value.filter((e) => e.status === "error");
      if (batchEntries.value.length === 0) {
        batchUrls.value = "";
      }
    } finally {
      isStarting.value = false;
    }
  }

  // ── Event handling ───────────────────────────────────────────────
  function handleDownloadUpdated(summary: DownloadSummary) {
    const existing = downloads.value.find((d) => d.id === summary.id);
    const oldState = existing?.state;

    if (oldState && oldState !== "failed" && summary.state === "failed") {
      listStore
        .getCallbacks()
        .onDownloadFailed?.(
          summary.fileName,
          summary.error ? toFriendlyError(summary.error) : t("common.unknown"),
        );
    }

    if (oldState && oldState !== "completed" && summary.state === "completed") {
      notify.notifySuccess(t("notifications.downloadComplete"));
    }

    listStore.upsertSummary(summary);

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
        chunks: summary.chunks,
      };
    }

    if (notificationsEnabled.value && oldState && oldState !== summary.state) {
      if (summary.state === "completed") {
        void fireNotification(
          t("notifications.downloadComplete"),
          t("notifications.downloadCompleteBody", { fileName: summary.fileName }),
          summary.id,
        );
      } else if (summary.state === "failed") {
        void fireNotification(
          t("notifications.downloadFailed"),
          t("notifications.downloadFailedBody", { fileName: summary.fileName }),
        );
      }
    }
  }

  let unlistenEvent: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenWarning: UnlistenFn | null = null;
  let unlistenFullState: UnlistenFn | null = null;
  let unlistenNotificationAction: (() => void) | null = null;
  let mounted = false;
  let btRuntimeTimer: ReturnType<typeof setInterval> | null = null;

  function startAutoRefresh() {
    mounted = true;

    void listen<DownloadProgress>("download-progress", (event) => {
      listStore.patchProgress(event.payload);
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

    void listen<{ id: string; message: string }>("download-warning", (event) => {
      notify.notifyWarning(event.payload.message);
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenWarning = unlisten;
    });

    void listen<DownloadSummary[]>("download-full-state", (event) => {
      downloads.value = event.payload;
      listStore.ensureSelection();
    }).then((unlisten) => {
      if (!mounted) {
        unlisten();
        return;
      }
      unlistenFullState = unlisten;
    });

    void onAction((notification) => {
      const downloadId = notification.extra?.downloadId;
      if (typeof downloadId === "string") {
        void openDownloadInExplorer(downloadId);
      }
    })
      .then((listener) => {
        if (!mounted) {
          listener.unregister();
          return;
        }
        unlistenNotificationAction = () => {
          listener.unregister();
        };
      })
      .catch(() => {});

    btRuntimeTimer = setInterval(() => {
      void btStore.refreshBtRuntimeStatus({ silent: true });
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

    if (unlistenWarning) {
      unlistenWarning();
      unlistenWarning = null;
    }

    if (unlistenFullState) {
      unlistenFullState();
      unlistenFullState = null;
    }

    if (unlistenNotificationAction) {
      unlistenNotificationAction();
      unlistenNotificationAction = null;
    }

    if (btRuntimeTimer) {
      clearInterval(btRuntimeTimer);
      btRuntimeTimer = null;
    }

    isAutoRefreshing.value = false;
  }

  // ── Lifecycle methods ────────────────────────────────────────────
  async function initStore() {
    await listStore.refreshList({ silent: true });
    await btStore.refreshBtRuntimeStatus({ silent: true });

    if (selectedId.value) {
      await refreshStatus(selectedId.value, { silent: true });
    }

    startAutoRefresh();
  }

  function destroyStore() {
    stopAutoRefresh();
  }

  return {
    // Callbacks / lifecycle
    configure,
    initStore,
    destroyStore,

    // State
    actionName,
    setNotificationsEnabled,
    setMessage,
    setError,
    canCancel,
    canPause,
    canResume,
    canPauseDownload: listStore.canPauseDownload,
    canResumeDownload: listStore.canResumeDownload,
    btRuntimeStatus,
    downloads,
    form,
    isAutoRefreshing,
    isPickingDirectory,
    isPickingTorrent,
    isRefreshingList,
    isRefreshingStatus,
    isStarting,

    // Form helpers
    applySchedulerDefaults: formStore.applySchedulerDefaults,
    applyAppSettingsDefaults: formStore.applyAppSettingsDefaults,
    pickDestinationDirectory: formStore.pickDestinationDirectory,
    pickTorrentSourceFile: formStore.pickTorrentSourceFile,
    probeSha256Checksum: formStore.probeSha256Checksum,

    // List
    refreshList: listStore.refreshList,
    refreshBtRuntimeStatus: btStore.refreshBtRuntimeStatus,
    refreshStatus,

    // Actions
    runCancel: listStore.runCancel,
    runDeleteTask: listStore.runDeleteTask,
    runDeleteTaskPermanently: listStore.runDeleteTaskPermanently,
    runCopyLink: listStore.runCopyLink,
    runOpenInExplorer: listStore.runOpenInExplorer,
    runPause: listStore.runPause,
    runPauseFor: listStore.runPauseFor,
    runResume: listStore.runResume,
    runResumeFor: listStore.runResumeFor,
    runPauseAll: listStore.runPauseAll,
    runResumeAll: listStore.runResumeAll,
    runClearCompleted: listStore.runClearCompleted,
    runBatchDelete: listStore.runBatchDelete,
    runBatchPause: listStore.runBatchPause,
    runBatchResume: listStore.runBatchResume,
    runBatchCancel: listStore.runBatchCancel,
    runSetPriority: listStore.runSetPriority,

    // Selection
    selectDownload,
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,

    // Form / submit
    submitStart,
    autoFillFromClipboard: formStore.autoFillFromClipboard,

    // Batch
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
    parseBatchUrls: formStore.parseBatchUrls,
    submitBatch,
    toggleBatchMode: formStore.toggleBatchMode,
  };
});
