import type { ComputedRef, Ref } from "vue";

import {
  cancelDownload,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
} from "../lib/tauri/download-api";
import { t } from "../i18n";
import { canPauseState, canResumeState, toMessage, toSummary } from "./downloadHelpers";
import type { DownloadSnapshot, DownloadSummary } from "../types/download";

export interface UseDownloadActionsInput {
  downloads: Ref<DownloadSummary[]>;
  selectedId: Ref<string | null>;
  selectedSnapshot: Ref<DownloadSnapshot | null>;
  actionName: Ref<string>;
  allowAutoSelect: Ref<boolean>;
  selectedSummary: ComputedRef<DownloadSummary | null>;
  selectedDownload: ComputedRef<DownloadSnapshot | DownloadSummary | null>;
  canPause: ComputedRef<boolean>;
  canResume: ComputedRef<boolean>;
  canCancel: ComputedRef<boolean>;
  upsertSummary: (summary: DownloadSummary) => void;
  removeSummary: (downloadId: string) => void;
  refreshStatus: (downloadId?: string | null, options?: { silent?: boolean }) => Promise<void>;
  setMessage: (message: string) => void;
  setError: (message: string) => void;
  clearMessage: () => void;
}

export function useDownloadActions(input: UseDownloadActionsInput) {
  const {
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
  } = input;

  async function selectDownload(downloadId: string | null) {
    allowAutoSelect.value = downloadId !== null;
    selectedId.value = downloadId;
    if (downloadId) {
      await refreshStatus(downloadId, { silent: true });
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

  async function runActionFor(
    downloadId: string | null,
    name: string,
    action: (downloadId: string) => Promise<DownloadSnapshot>,
  ) {
    if (!downloadId) {
      return;
    }

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

  interface BatchActionConfig {
    actionNameValue: string;
    items: Array<{ id: string; fileName: string }>;
    apiCall: (id: string) => Promise<DownloadSnapshot>;
    successMessageKey: string;
    onSuccess: (id: string, snapshot: DownloadSnapshot) => void;
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
      onSuccess: (_id, snapshot) => { upsertSummary(toSummary(snapshot)); },
    });
  }

  async function runResumeAll() {
    const toResume = downloads.value.filter((d) => d.state === "paused");
    await runBatchAction({
      actionNameValue: "BatchResume",
      items: toResume,
      apiCall: resumeDownload,
      successMessageKey: "messages.resumedAll",
      onSuccess: (_id, snapshot) => { upsertSummary(toSummary(snapshot)); },
    });
  }

  async function runClearCompleted() {
    const toClear = downloads.value.filter((d) => d.state === "completed");
    await runBatchAction({
      actionNameValue: "BatchClear",
      items: toClear,
      apiCall: removeDownload,
      successMessageKey: "messages.clearedCompleted",
      onSuccess: (id) => { removeSummary(id); },
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
      onSuccess: (id) => { removeSummary(id); },
    });
  }

  return {
    actionName,
    allowAutoSelect,
    canPause,
    canResume,
    canCancel,
    canPauseDownload: (download: DownloadSummary) => canPauseState(download.state),
    canResumeDownload: (download: DownloadSummary) => canResumeState(download.state),
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,
    selectDownload,
    runCancel: () => runAction("Cancel", cancelDownload),
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
  };
}
