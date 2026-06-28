import { reactive, ref, type Ref } from "vue";

import { startDownload } from "../lib/tauri/download-api";
import { pickDirectory, pickMetalinkFile, pickTorrentFile } from "../lib/tauri/dialog-api";
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
  const isPickingMetalink = ref(false);
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

    if (form.kind === "bt" || form.kind === "metalink") {
      if (form.downloadLimitBps !== null && form.downloadLimitBps > 0) {
        request.downloadLimitBps = form.downloadLimitBps;
      }
      if (form.uploadLimitBps !== null && form.uploadLimitBps > 0) {
        request.uploadLimitBps = form.uploadLimitBps;
      }
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

  return {
    form,
    isStarting,
    isPickingDirectory,
    isPickingMetalink,
    isPickingTorrent,
    applySchedulerDefaults,
    applyAppSettingsDefaults,
    buildStartRequest,
    pickDestinationDirectory,
    pickMetalinkSourceFile,
    pickTorrentSourceFile,
    submitStart,
  };
}
