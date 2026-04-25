import { computed, onMounted, onUnmounted, reactive, ref } from "vue";

import {
  cancelDownload,
  getDownloadStatus,
  listDownloads,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
  startDownload,
} from "../lib/tauri/download-api";
import { pickDirectory, pickMetalinkFile, pickTorrentFile } from "../lib/tauri/dialog-api";
import { t } from "../i18n";
import type {
  ChecksumMode,
  DownloadFormState,
  DownloadSnapshot,
  DownloadState,
  DownloadSummary,
  StartDownloadRequest,
} from "../types/download";
import type { AppSettings, SchedulerMode } from "../types/settings";

const terminalStates: DownloadState[] = ["completed", "failed", "canceled"];
const autoRefreshIntervalMs = 1500;

function canPauseState(state?: DownloadState | null) {
  return Boolean(state && ["queued", "downloading", "retrying", "verifying"].includes(state));
}

function canResumeState(state?: DownloadState | null) {
  return state === "paused" || state === "failed";
}

function toMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function toSummary(snapshot: DownloadSnapshot): DownloadSummary {
  return {
    id: snapshot.id,
    kind: snapshot.kind,
    state: snapshot.state,
    url: snapshot.url,
    fileName: snapshot.fileName,
    destinationPath: snapshot.destinationPath,
    totalBytes: snapshot.totalBytes,
    downloadedBytes: snapshot.downloadedBytes,
    connectionCount: snapshot.connectionCount,
    threadMode: snapshot.threadMode,
    requestedThreadCount: snapshot.requestedThreadCount,
    desiredThreadCount: snapshot.desiredThreadCount,
    allocatedThreadCount: snapshot.allocatedThreadCount,
    adaptiveProfile: snapshot.adaptiveProfile,
    threadNote: snapshot.threadNote,
    speedBytesPerSecond: snapshot.speedBytesPerSecond,
    etaSeconds: snapshot.etaSeconds,
    uploadedBytes: snapshot.uploadedBytes,
    peerCount: snapshot.peerCount,
    uploadStatus: snapshot.uploadStatus,
    error: snapshot.error,
  };
}

export function useDownloader() {
  const form = reactive<DownloadFormState>({
    kind: "http",
    url: "",
    destinationDir: "",
    fileName: "",
    userAgent: "",
    threadMode: "adaptive",
    threadCount: 8,
    maxRetries: 5,
    checksum: "blake3" as ChecksumMode,
  });

  const downloads = ref<DownloadSummary[]>([]);
  const selectedId = ref<string | null>(null);
  const selectedSnapshot = ref<DownloadSnapshot | null>(null);
  const errorMessage = ref<string>("");
  const infoMessage = ref<string>("");
  const isStarting = ref(false);
  const isRefreshingList = ref(false);
  const isRefreshingStatus = ref(false);
  const isPickingDirectory = ref(false);
  const isPickingMetalink = ref(false);
  const isPickingTorrent = ref(false);
  const isAutoRefreshing = ref(false);
  const actionName = ref("");
  const allowAutoSelect = ref(true);

  let autoRefreshTimer: ReturnType<typeof setInterval> | null = null;
  let autoRefreshInFlight = false;

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

  function setMessage(message: string) {
    infoMessage.value = message;
    errorMessage.value = "";
  }

  function setError(message: string) {
    errorMessage.value = message;
    infoMessage.value = "";
  }

  function clearMessage() {
    errorMessage.value = "";
    infoMessage.value = "";
  }

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

  function applySchedulerDefaults(mode: SchedulerMode, maxThreadsPerTask?: number) {
    if (form.kind !== "http") {
      return;
    }

    if (mode === "automatic") {
      form.threadMode = "adaptive";
      if (
        typeof maxThreadsPerTask === "number" &&
        Number.isFinite(maxThreadsPerTask) &&
        form.threadCount &&
        form.threadCount > maxThreadsPerTask
      ) {
        form.threadCount = maxThreadsPerTask;
      }
      return;
    }

    form.threadMode = "fixed";
    if (
      typeof maxThreadsPerTask === "number" &&
      Number.isFinite(maxThreadsPerTask) &&
      (!form.threadCount || form.threadCount > maxThreadsPerTask)
    ) {
      form.threadCount = maxThreadsPerTask;
      return;
    }

    if (!form.threadCount) {
      form.threadCount = 8;
    }
  }

  function applyAppSettingsDefaults(settings: AppSettings) {
    if (settings.appearance?.themeColor) {
      document.documentElement.dataset.theme = settings.appearance.themeColor;
    }
    if (
      (form.kind === "metalink" && !settings.download.enableMetalink) ||
      (form.kind === "sftp" && !settings.download.enableSftp)
    ) {
      form.kind = "http";
    }
    form.destinationDir = settings.download.defaultDownloadDir;
    form.maxRetries = settings.download.defaultMaxRetries;
    form.checksum = settings.download.defaultChecksum;
    form.userAgent = settings.download.defaultUserAgent;
    applySchedulerDefaults(settings.scheduler.mode, settings.scheduler.automatic.maxThreadsPerTask);
  }

  function buildStartRequest(): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: form.kind,
      url: form.url.trim(),
      destinationDir: form.destinationDir.trim(),
    };

    if (form.kind === "http" || form.kind === "metalink" || form.kind === "sftp") {
      request.threadMode = form.threadMode;

      if (form.kind === "http") {
        const fileName = form.fileName.trim();
        if (fileName) {
          request.fileName = fileName;
        }
      }

      const userAgent = form.userAgent.trim();
      if (userAgent) {
        request.userAgent = userAgent;
      }

      if (typeof form.threadCount === "number" && Number.isFinite(form.threadCount)) {
        const threadCount = Math.trunc(form.threadCount);

        if (threadCount > 0) {
          request.threadCount = threadCount;
        }
      }

      if (typeof form.maxRetries === "number" && Number.isFinite(form.maxRetries)) {
        const maxRetries = Math.trunc(form.maxRetries);

        if (maxRetries >= 0) {
          request.maxRetries = maxRetries;
        }
      }

      if (form.kind === "http") {
        request.checksum = form.checksum;
      }
    }

    return request;
  }

  async function refreshList(options?: { silent?: boolean }) {
    if (isRefreshingList.value) {
      return;
    }

    isRefreshingList.value = true;

    try {
      downloads.value = await listDownloads();
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

  async function pickDestinationDirectory() {
    if (isPickingDirectory.value) {
      return;
    }

    isPickingDirectory.value = true;

    try {
      const selectedPath = await pickDirectory();

      if (selectedPath) {
        form.destinationDir = selectedPath;
      }
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isPickingDirectory.value = false;
    }
  }

  async function pickTorrentSourceFile() {
    if (isPickingTorrent.value) {
      return;
    }

    isPickingTorrent.value = true;

    try {
      const selectedPath = await pickTorrentFile();

      if (selectedPath) {
        form.kind = "bt";
        form.url = selectedPath;
      }
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isPickingTorrent.value = false;
    }
  }

  async function pickMetalinkSourceFile() {
    if (isPickingMetalink.value) {
      return;
    }

    isPickingMetalink.value = true;

    try {
      const selectedPath = await pickMetalinkFile();

      if (selectedPath) {
        form.kind = "metalink";
        form.url = selectedPath;
      }
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isPickingMetalink.value = false;
    }
  }

  async function runAutoRefresh() {
    if (autoRefreshInFlight || isStarting.value || Boolean(actionName.value)) {
      return;
    }

    autoRefreshInFlight = true;
    isAutoRefreshing.value = true;

    try {
      await refreshList({ silent: true });

      if (shouldRefreshSelectedStatus()) {
        await refreshStatus(selectedId.value, { silent: true });
      }
    } finally {
      isAutoRefreshing.value = false;
      autoRefreshInFlight = false;
    }
  }

  function startAutoRefresh() {
    if (autoRefreshTimer) {
      return;
    }

    autoRefreshTimer = setInterval(() => {
      void runAutoRefresh();
    }, autoRefreshIntervalMs);
  }

  function stopAutoRefresh() {
    if (autoRefreshTimer) {
      clearInterval(autoRefreshTimer);
      autoRefreshTimer = null;
    }

    autoRefreshInFlight = false;
    isAutoRefreshing.value = false;
  }

  async function selectDownload(downloadId: string | null) {
    allowAutoSelect.value = downloadId !== null;
    selectedId.value = downloadId;
    if (downloadId) {
      await refreshStatus(downloadId, { silent: true });
    } else {
      selectedSnapshot.value = null;
    }
  }

  async function submitStart() {
    if (!form.url.trim() || !form.destinationDir.trim()) {
      setError(
        form.kind === "bt"
          ? t("messages.torrentStartRequired")
          : form.kind === "metalink"
            ? t("messages.metalinkStartRequired")
            : form.kind === "sftp"
              ? t("messages.sftpStartRequired")
              : t("messages.startRequired"),
      );
      return;
    }

    isStarting.value = true;

    try {
      clearMessage();

      const downloadId = await startDownload(buildStartRequest());
      allowAutoSelect.value = true;
      selectedId.value = downloadId;
      await refreshList();
      await refreshStatus(downloadId, { silent: true });
      setMessage(t("messages.downloadQueued", { id: downloadId }));
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isStarting.value = false;
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

  onMounted(async () => {
    await refreshList({ silent: true });

    if (selectedId.value) {
      await refreshStatus(selectedId.value, { silent: true });
    }

    startAutoRefresh();
  });

  onUnmounted(() => {
    stopAutoRefresh();
  });

  return {
    actionName,
    canCancel,
    canPause,
    canResume,
    canPauseDownload: (download: DownloadSummary) => canPauseState(download.state),
    canResumeDownload: (download: DownloadSummary) => canResumeState(download.state),
    downloads,
    errorMessage,
    form,
    infoMessage,
    isAutoRefreshing,
    isPickingDirectory,
    isPickingMetalink,
    isPickingTorrent,
    isRefreshingList,
    isRefreshingStatus,
    isStarting,
    applySchedulerDefaults,
    applyAppSettingsDefaults,
    pickDestinationDirectory,
    pickMetalinkSourceFile,
    pickTorrentSourceFile,
    refreshList,
    refreshStatus,
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
    selectDownload,
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,
    submitStart,
  };
}
