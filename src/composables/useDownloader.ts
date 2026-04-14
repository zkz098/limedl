import { computed, onMounted, onUnmounted, reactive, ref } from "vue";

import {
  cancelDownload,
  getDownloadStatus,
  listDownloads,
  pauseDownload,
  resumeDownload,
  startDownload,
} from "../lib/tauri/download-api";
import { pickDirectory } from "../lib/tauri/dialog-api";
import type {
  ChecksumMode,
  DownloadFormState,
  DownloadSnapshot,
  DownloadState,
  DownloadSummary,
  StartDownloadRequest,
} from "../types/download";

const terminalStates: DownloadState[] = ["completed", "failed", "canceled"];
const autoRefreshIntervalMs = 1500;

function toMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function toSummary(snapshot: DownloadSnapshot): DownloadSummary {
  return {
    id: snapshot.id,
    state: snapshot.state,
    fileName: snapshot.fileName,
    destinationPath: snapshot.destinationPath,
    totalBytes: snapshot.totalBytes,
    downloadedBytes: snapshot.downloadedBytes,
    connectionCount: snapshot.connectionCount,
    speedBytesPerSecond: snapshot.speedBytesPerSecond,
    etaSeconds: snapshot.etaSeconds,
    error: snapshot.error,
  };
}

export function useDownloader() {
  const form = reactive<DownloadFormState>({
    url: "",
    destinationDir: "",
    fileName: "",
    maxConnections: 8,
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
  const isAutoRefreshing = ref(false);
  const actionName = ref("");

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
    const state = selectedDownload.value?.state;

    if (!state) {
      return false;
    }

    return ["queued", "downloading", "retrying", "verifying"].includes(state);
  });

  const canResume = computed(() => selectedDownload.value?.state === "paused");

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

  function ensureSelection() {
    if (selectedId.value && downloads.value.some((download) => download.id === selectedId.value)) {
      return;
    }

    selectedId.value = downloads.value[0]?.id ?? null;

    if (!selectedId.value) {
      selectedSnapshot.value = null;
    }
  }

  function buildStartRequest(): StartDownloadRequest {
    const request: StartDownloadRequest = {
      url: form.url.trim(),
      destinationDir: form.destinationDir.trim(),
    };

    const fileName = form.fileName.trim();
    if (fileName) {
      request.fileName = fileName;
    }

    if (typeof form.maxConnections === "number" && Number.isFinite(form.maxConnections)) {
      const maxConnections = Math.trunc(form.maxConnections);

      if (maxConnections > 0) {
        request.maxConnections = maxConnections;
      }
    }

    if (typeof form.maxRetries === "number" && Number.isFinite(form.maxRetries)) {
      const maxRetries = Math.trunc(form.maxRetries);

      if (maxRetries >= 0) {
        request.maxRetries = maxRetries;
      }
    }

    request.checksum = form.checksum;

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

      if (selectedId.value && selectedSnapshot.value && selectedSnapshot.value.id !== selectedId.value) {
        selectedSnapshot.value = null;
      }

      if (!downloads.value.length && !options?.silent) {
        setMessage("No downloads yet. Start one from the form.");
      } else if (!options?.silent && downloads.value.length) {
        setMessage(`Queue refreshed. ${downloads.value.length} item(s) loaded.`);
      }
    } catch (error) {
      if (!options?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingList.value = false;
    }
  }

  async function refreshStatus(
    downloadId = selectedId.value,
    options?: { silent?: boolean },
  ) {
    if (!downloadId) {
      return;
    }

    if (isRefreshingStatus.value) {
      return;
    }

    isRefreshingStatus.value = true;

    try {
      const snapshot = await getDownloadStatus(downloadId);
      selectedId.value = snapshot.id;
      selectedSnapshot.value = snapshot;
      upsertSummary(toSummary(snapshot));

      if (!options?.silent) {
        setMessage(`Status refreshed for ${snapshot.fileName}.`);
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
    selectedId.value = downloadId;
    if (downloadId) {
      await refreshStatus(downloadId, { silent: true });
    }
  }

  async function submitStart() {
    if (!form.url.trim() || !form.destinationDir.trim()) {
      setError("URL and destination directory are required.");
      return;
    }

    isStarting.value = true;

    try {
      clearMessage();

      const downloadId = await startDownload(buildStartRequest());
      selectedId.value = downloadId;
      await refreshList();
      await refreshStatus(downloadId, { silent: true });
      setMessage(`Download queued with id ${downloadId}.`);
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
    if (!selectedId.value) {
      return;
    }

    actionName.value = name;

    try {
      clearMessage();

      const snapshot = await action(selectedId.value);
      selectedSnapshot.value = snapshot;
      upsertSummary(toSummary(snapshot));
      setMessage(`${name} complete for ${snapshot.fileName}.`);
    } catch (error) {
      setError(toMessage(error));
    } finally {
      actionName.value = "";
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
    downloads,
    errorMessage,
    form,
    infoMessage,
    isAutoRefreshing,
    isPickingDirectory,
    isRefreshingList,
    isRefreshingStatus,
    isStarting,
    pickDestinationDirectory,
    refreshList,
    refreshStatus,
    runCancel: () => runAction("Cancel", cancelDownload),
    runPause: () => runAction("Pause", pauseDownload),
    runResume: () => runAction("Resume", resumeDownload),
    selectDownload,
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,
    submitStart,
  };
}
