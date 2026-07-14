import { reactive, ref, type Ref } from "vue";

import { setBtSpeedLimit, startDownload } from "../lib/tauri/download-api";
import { pickDirectory, pickTorrentFile } from "../lib/tauri/dialog-api";
import { t } from "../i18n";
import { toMessage } from "./downloadHelpers";
import type {
  ChecksumMode,
  DownloadFormState,
  DownloadSummary,
  StartDownloadRequest,
} from "../types/download";
import type { AppSettings, SchedulerMode } from "../types/settings";

export interface UseDownloadFormInput {
  selectedId: Ref<string | null>;
  allowAutoSelect: Ref<boolean>;
  isStarting: Ref<boolean>;
  upsertSummary: (summary: DownloadSummary) => void;
  refreshList: (options?: { silent?: boolean }) => Promise<void>;
  refreshStatus: (downloadId?: string | null, options?: { silent?: boolean }) => Promise<void>;
  setMessage: (message: string) => void;
  setError: (message: string) => void;
  clearMessage: () => void;
}

export function useDownloadForm(input: UseDownloadFormInput) {
  const {
    selectedId,
    allowAutoSelect,
    isStarting,
    refreshList,
    refreshStatus,
    setMessage,
    setError,
    clearMessage,
  } = input;

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
    downloadLimitBps: null,
    uploadLimitBps: null,
  });

  const isPickingDirectory = ref(false);
  const isPickingTorrent = ref(false);

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
    form.destinationDir = settings.download.defaultDownloadDir;
    form.maxRetries = settings.download.defaultMaxRetries;
    form.checksum = settings.download.defaultChecksum;
    form.userAgent = settings.download.defaultUserAgent;
    applySchedulerDefaults(settings.scheduler.mode, settings.scheduler.automatic.maxThreadsPerTask);
    // TODO: Pre-fill form.downloadLimitBps / form.uploadLimitBps from
    // settings.bt.defaultDownloadSpeedLimit / defaultUploadSpeedLimit
    // once those fields are added to the BtSettings interface in settings.ts.
  }

  function buildStartRequest(): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: form.kind,
      url: form.url.trim(),
      destinationDir: form.destinationDir.trim(),
    };

    if (form.kind === "http") {
      request.threadMode = form.threadMode;

      const fileName = form.fileName.trim();
      if (fileName) {
        request.fileName = fileName;
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

      request.checksum = form.checksum;
    }

    if (form.kind === "bt") {
      if (form.selectedFileIndices && form.selectedFileIndices.length > 0) {
        request.selectedFileIndices = [...form.selectedFileIndices];
      }
      if (form.startPaused === true) {
        request.startPaused = true;
      }
    }

    return request;
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

  async function submitStart() {
    if (!form.url.trim() || !form.destinationDir.trim()) {
      setError(
        form.kind === "bt" ? t("messages.torrentStartRequired") : t("messages.startRequired"),
      );
      return;
    }

    isStarting.value = true;

    try {
      clearMessage();

      const downloadId = await startDownload(buildStartRequest());
      allowAutoSelect.value = true;
      selectedId.value = downloadId;

      // For BT downloads, apply initial per-download speed limits after start
      // (the StartDownloadRequest no longer carries these fields).
      if (form.kind === "bt") {
        const dl = form.downloadLimitBps;
        const ul = form.uploadLimitBps;
        if ((dl !== null && dl > 0) || (ul !== null && ul > 0)) {
          try {
            await setBtSpeedLimit(downloadId, dl ?? undefined, ul ?? undefined);
          } catch {
            // Non-critical — speed limit is optional, the download is already running.
          }
        }
      }

      await refreshList();
      await refreshStatus(downloadId, { silent: true });
      setMessage(t("messages.downloadQueued", { id: downloadId }));
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isStarting.value = false;
    }
  }

  /**
   * Reads the system clipboard and auto-fills the URL field if it contains
   * a valid download URL (http, https, or magnet link). Also sets the
   * appropriate protocol kind (http → "http", magnet → "bt").
   *
   * Silently ignores non-URL content and clipboard errors (no permission, etc.).
   */
  async function autoFillFromClipboard(): Promise<void> {
    try {
      const text = await navigator.clipboard.readText();
      if (!text) return;

      const trimmed = text.trim();

      if (trimmed.startsWith("magnet:?")) {
        form.kind = "bt";
        form.url = trimmed;
      } else if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
        form.kind = "http";
        form.url = trimmed;
      }
      // Other content — not a recognized download URL, do nothing.
    } catch {
      // Clipboard read failed (e.g. permission denied, WebView not focused).
      // This is non-critical — silently ignore.
    }
  }

  return {
    form,
    isStarting,
    isPickingDirectory,
    isPickingTorrent,
    applySchedulerDefaults,
    applyAppSettingsDefaults,
    buildStartRequest,
    pickDestinationDirectory,
    pickTorrentSourceFile,
    submitStart,
    autoFillFromClipboard,
  };
}
