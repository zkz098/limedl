import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  cancelDownload,
  listDownloads,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
  setPriority,
} from "../../lib/tauri/download-api";
import { t } from "../../i18n";
import {
  canPauseState,
  canResumeState,
  terminalStates,
  toMessage,
  toSummary,
} from "../../composables/downloadHelpers";
import { useNotificationStore } from "../notification";
import type {
  DownloadProgress,
  DownloadSnapshot,
  DownloadSummary,
  Priority,
} from "../../types/download";
import type { BatchActionConfig, DownloadStoreOptions } from "./types";
import {
  applyProgressToSnapshot,
  applyProgressToSummary,
  canPauseDownload,
  canResumeDownload,
} from "./helpers";

export const useDownloadListStore = defineStore("downloadList", () => {
  // ── Callbacks ────────────────────────────────────────────────────
  let callbacks: DownloadStoreOptions = {};
  function configure(opts: DownloadStoreOptions) {
    callbacks = opts;
  }
  function getCallbacks() {
    return callbacks;
  }

  // ── Notification store ───────────────────────────────────────────
  const notify = useNotificationStore();

  // ── Core reactive state ──────────────────────────────────────────
  const downloads = ref<DownloadSummary[]>([]);
  const downloadMap = new Map<string, DownloadSummary>();

  function syncDownloadMap(items: DownloadSummary[]) {
    downloadMap.clear();
    for (const item of items) {
      downloadMap.set(item.id, item);
    }
  }

  const selectedId = ref<string | null>(null);
  const selectedSnapshot = ref<DownloadSnapshot | null>(null);
  const isAutoRefreshing = ref(false);
  const allowAutoSelect = ref(true);
  const actionName = ref("");
  const notificationsEnabled = ref(false);
  const isRefreshingList = ref(false);

  // ── Message helpers ──────────────────────────────────────────────
  function setMessage(message: string) {
    notify.notifyInfo(message);
  }

  function setError(message: string) {
    notify.notifyError(message);
  }

  function clearMessage() {
    notify.clearAll();
  }

  // ── upsertSummary / patchProgress / removeSummary ───────────────
  function upsertSummary(summary: DownloadSummary) {
    const next = [...downloads.value];
    const index = next.findIndex((download) => download.id === summary.id);

    if (index >= 0) {
      next[index] = summary;
    } else {
      next.unshift(summary);
    }

    downloads.value = next;
    downloadMap.set(summary.id, summary);
  }

  function patchProgress(progress: DownloadProgress) {
    let existing = downloadMap.get(progress.id);
    if (!existing) {
      existing = downloads.value.find((d) => d.id === progress.id);
      if (existing) {
        downloadMap.set(existing.id, existing);
      }
    }
    if (!existing) return;

    applyProgressToSummary(existing, progress);

    if (selectedId.value === progress.id && selectedSnapshot.value) {
      applyProgressToSnapshot(selectedSnapshot.value, progress);
    }
  }

  function removeSummary(downloadId: string) {
    downloads.value = downloads.value.filter((download) => download.id !== downloadId);
    downloadMap.delete(downloadId);

    callbacks.onDownloadsRemoved?.([downloadId]);

    if (selectedId.value === downloadId) {
      allowAutoSelect.value = false;
      selectedId.value = null;
      selectedSnapshot.value = null;
    }
  }

  function ensureSelection() {
    if (selectedId.value && (downloadMap.has(selectedId.value) || downloads.value.some((download) => download.id === selectedId.value))) {
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

    return downloadMap.get(selectedId.value) ?? downloads.value.find((download) => download.id === selectedId.value) ?? null;
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

  // ── List operations ──────────────────────────────────────────────
  async function refreshList(options?: { silent?: boolean }) {
    if (isRefreshingList.value) return;

    isRefreshingList.value = true;

    try {
      const oldIds = new Set(downloads.value.map((d) => d.id));
      downloads.value = await listDownloads();
      syncDownloadMap(downloads.value);
      const newIds = new Set(downloads.value.map((d) => d.id));
      const removedIds = [...oldIds].filter((id) => !newIds.has(id));
      if (removedIds.length > 0) {
        callbacks.onDownloadsRemoved?.(removedIds);
      }
      ensureSelection();

      if (
        selectedId.value &&
        selectedSnapshot.value &&
        selectedSnapshot.value.id !== selectedId.value
      ) {
        selectedSnapshot.value = null;
      }

      if (!downloads.value.length && !options?.silent) {
        setMessage(t("messages.noDownloads"));
      } else if (!options?.silent && downloads.value.length) {
        setMessage(t("messages.queueRefreshed", { count: downloads.value.length }));
      }
    } catch (error) {
      if (!options?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingList.value = false;
    }
  }

  // ── Actions ──────────────────────────────────────────────────────
  async function selectDownload(
    downloadId: string | null,
    refreshStatusFn?: (id: string, opts?: { silent?: boolean }) => Promise<void>,
  ) {
    allowAutoSelect.value = downloadId !== null;
    selectedId.value = downloadId;
    if (downloadId) {
      if (refreshStatusFn) {
        await refreshStatusFn(downloadId, { silent: true });
      }
    } else {
      selectedSnapshot.value = null;
    }
  }

  async function runAction(
    name: string,
    action: (downloadId: string) => Promise<DownloadSnapshot>,
  ) {
    await runActionFor(selectedId.value, name, action);
  }

  const runCancel = () => runAction("Cancel", cancelDownload);

  async function runActionFor(
    downloadId: string | null,
    name: string,
    action: (downloadId: string) => Promise<DownloadSnapshot>,
  ) {
    if (!downloadId) return;

    actionName.value = name;

    try {
      clearMessage();

      const snapshot = await action(downloadId);

      if (name === "Cancel") {
        removeSummary(downloadId);
      } else {
        if (selectedId.value === downloadId) {
          selectedSnapshot.value = snapshot;
        }
        upsertSummary(toSummary(snapshot));
      }

      setMessage(
        t("messages.actionComplete", {
          action: t(`actions.${name}`),
          fileName: snapshot.fileName,
        }),
      );
    } catch (error) {
      setError(toMessage(error));
    } finally {
      actionName.value = "";
    }
  }

  async function runTaskMaintenance(
    downloadId: string,
    name: string,
    action: (downloadId: string) => Promise<DownloadSnapshot>,
  ) {
    actionName.value = name;

    try {
      clearMessage();

      const snapshot = await action(downloadId);
      removeSummary(downloadId);

      setMessage(
        t("messages.actionComplete", {
          action: t(`actions.${name}`),
          fileName: snapshot.fileName,
        }),
      );
    } catch (error) {
      setError(toMessage(error));
    } finally {
      actionName.value = "";
    }
  }

  async function runOpenInExplorer(downloadId: string) {
    actionName.value = "OpenInExplorer";

    try {
      clearMessage();
      await openDownloadInExplorer(downloadId);
      setMessage(t("messages.openedInExplorer"));
    } catch (error) {
      setError(toMessage(error));
    } finally {
      actionName.value = "";
    }
  }

  async function runCopyLink(downloadId: string) {
    const target =
      selectedSnapshot.value?.id === downloadId
        ? selectedSnapshot.value
        : downloads.value.find((download) => download.id === downloadId);

    if (!target?.url) {
      setError(t("messages.copyLinkFailed"));
      return;
    }

    try {
      await navigator.clipboard.writeText(target.url);
      setMessage(t("messages.linkCopied"));
    } catch (error) {
      setError(toMessage(error));
    }
  }

  async function runBatchAction(config: BatchActionConfig) {
    actionName.value = config.actionNameValue;
    clearMessage();

    if (config.items.length === 0) {
      setMessage(t(config.successMessageKey, { count: 0 }));
      actionName.value = "";
      return;
    }

    const results = await Promise.allSettled(config.items.map((item) => config.apiCall(item.id)));
    let successCount = 0;
    const errorMessages: string[] = [];

    results.forEach((result, i) => {
      if (result.status === "fulfilled") {
        successCount++;
        config.onSuccess(config.items[i].id, result.value);
      } else {
        errorMessages.push(`${config.items[i].fileName}: ${toMessage(result.reason)}`);
      }
    });

    setMessage(t(config.successMessageKey, { count: successCount }));
    if (errorMessages.length > 0) {
      setError(errorMessages.join("; "));
    }
    actionName.value = "";
  }

  async function runPauseAll() {
    const toPause = downloads.value.filter((d) => d.state === "downloading");
    await runBatchAction({
      actionNameValue: "BatchPause",
      items: toPause,
      apiCall: pauseDownload,
      successMessageKey: "messages.pausedAll",
      onSuccess: (_id, snapshot) => {
        upsertSummary(toSummary(snapshot));
      },
    });
  }

  async function runResumeAll() {
    const toResume = downloads.value.filter((d) => d.state === "paused");
    await runBatchAction({
      actionNameValue: "BatchResume",
      items: toResume,
      apiCall: resumeDownload,
      successMessageKey: "messages.resumedAll",
      onSuccess: (_id, snapshot) => {
        upsertSummary(toSummary(snapshot));
      },
    });
  }

  async function runClearCompleted() {
    const toClear = downloads.value.filter((d) => d.state === "completed");
    await runBatchAction({
      actionNameValue: "BatchClear",
      items: toClear,
      apiCall: removeDownload,
      successMessageKey: "messages.clearedCompleted",
      onSuccess: (id) => {
        removeSummary(id);
      },
    });
  }

  async function runBatchDelete(downloadIds: string[]) {
    if (downloadIds.length === 0) return;
    const items = downloadIds.map((id) => ({
      id,
      fileName: downloads.value.find((d) => d.id === id)?.fileName ?? id,
    }));
    await runBatchAction({
      actionNameValue: "BatchDelete",
      items,
      apiCall: removeDownload,
      successMessageKey: "messages.batchDeleted",
      onSuccess: (id) => {
        removeSummary(id);
      },
    });
  }

  async function runBatchPause(downloadIds: string[]) {
    if (downloadIds.length === 0) return;
    const items = downloadIds.reduce<Array<{ id: string; fileName: string }>>((acc, id) => {
      const d = downloads.value.find((x) => x.id === id);
      if (d && canPauseState(d.state)) {
        acc.push({ id: d.id, fileName: d.fileName });
      }
      return acc;
    }, []);
    await runBatchAction({
      actionNameValue: "BatchPause",
      items,
      apiCall: pauseDownload,
      successMessageKey: "messages.batchPaused",
      onSuccess: (_id, snapshot) => {
        upsertSummary(toSummary(snapshot));
      },
    });
  }

  async function runBatchResume(downloadIds: string[]) {
    if (downloadIds.length === 0) return;
    const items = downloadIds.reduce<Array<{ id: string; fileName: string }>>((acc, id) => {
      const d = downloads.value.find((x) => x.id === id);
      if (d && canResumeState(d.state)) {
        acc.push({ id: d.id, fileName: d.fileName });
      }
      return acc;
    }, []);
    await runBatchAction({
      actionNameValue: "BatchResume",
      items,
      apiCall: resumeDownload,
      successMessageKey: "messages.batchResumed",
      onSuccess: (_id, snapshot) => {
        upsertSummary(toSummary(snapshot));
      },
    });
  }

  async function runBatchCancel(downloadIds: string[]) {
    if (downloadIds.length === 0) return;
    const items = downloadIds.reduce<Array<{ id: string; fileName: string }>>((acc, id) => {
      const d = downloads.value.find((x) => x.id === id);
      if (d && !terminalStates.includes(d.state)) {
        acc.push({ id: d.id, fileName: d.fileName });
      }
      return acc;
    }, []);
    await runBatchAction({
      actionNameValue: "BatchCancel",
      items,
      apiCall: cancelDownload,
      successMessageKey: "messages.batchCanceled",
      onSuccess: (id) => {
        removeSummary(id);
      },
    });
  }

  async function runSetPriority(downloadId: string, priority: Priority) {
    try {
      clearMessage();
      await setPriority(downloadId, priority);
      const summary = downloads.value.find((d) => d.id === downloadId);
      if (summary) {
        upsertSummary({ ...summary, priority });
      }
      setMessage(
        t("messages.actionComplete", {
          action: t("actions.SetPriority"),
          fileName: summary?.fileName ?? downloadId,
        }),
      );
    } catch (error) {
      setError(toMessage(error));
    }
  }

  function setNotificationsEnabled(enabled: boolean) {
    notificationsEnabled.value = enabled;
  }

  return {
    configure,
    getCallbacks,
    actionName,
    setNotificationsEnabled,
    notificationsEnabled,
    setMessage,
    setError,
    clearMessage,
    canCancel,
    canPause,
    canResume,
    canPauseDownload,
    canResumeDownload,
    downloads,
    isAutoRefreshing,
    isRefreshingList,
    refreshList,
    runCancel,
    runDeleteTask: (downloadId: string) => runTaskMaintenance(downloadId, "Delete", removeDownload),
    runDeleteTaskPermanently: (downloadId: string) =>
      runTaskMaintenance(downloadId, "Purge", purgeDownload),
    runCopyLink,
    runOpenInExplorer,
    runPause: () => runAction("Pause", pauseDownload),
    runPauseFor: (downloadId: string) => runActionFor(downloadId, "Pause", pauseDownload),
    runResume: () => runAction("Resume", resumeDownload),
    runResumeFor: (downloadId: string) => runActionFor(downloadId, "Resume", resumeDownload),
    runPauseAll,
    runResumeAll,
    runClearCompleted,
    runBatchDelete,
    runBatchPause,
    runBatchResume,
    runBatchCancel,
    runSetPriority,
    selectDownload,
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,
    allowAutoSelect,
    ensureSelection,
    upsertSummary,
    patchProgress,
    removeSummary,
  };
});
