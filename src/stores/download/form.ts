import { ref } from "vue";
import { defineStore } from "pinia";
import { pickDirectory, pickTorrentFile } from "../../lib/tauri/dialog-api";
import { readClipboardText } from "../../lib/platform";
import { probeChecksum, setBtSpeedLimit, startDownload } from "../../lib/tauri/download-api";
import { t } from "../../i18n";
import { toMessage } from "../../composables/downloadHelpers";
import { detectKindFromUrl, extractFileNameFromUrl } from "../../lib/url-utils";
import { useNotificationStore } from "../notification";
import type { AppSettings, SchedulerMode } from "../../types/settings";
import type {
  BatchSubmitProgress,
  BatchUrlEntry,
  DownloadFormState,
  StartDownloadRequest,
} from "../../types/download";
import { expandUrlRanges } from "./helpers";

const DEFAULT_THREAD_COUNT = 8;
const MAX_THREADS = 128;

export const useDownloadFormStore = defineStore("downloadForm", () => {
  const notify = useNotificationStore();

  const form = ref<DownloadFormState>({
    kind: "http",
    url: "",
    destinationDir: "",
    fileName: "",
    userAgent: "",
    threadMode: "adaptive",
    threadCount: DEFAULT_THREAD_COUNT,
    maxRetries: 5,
    checksum: "blake3",
    expectedChecksum: "",
    isProbingChecksum: false,
    checksumDetected: false,
    downloadLimitBps: null,
    uploadLimitBps: null,
  });

  const isPickingDirectory = ref(false);
  const isPickingTorrent = ref(false);
  const isFileNameLocked = ref(false);
  const autoDetectFileName = ref(true);
  const isStarting = ref(false);

  // ── Batch import mode ────────────────────────────────────────────
  const batchMode = ref(false);
  const batchUrls = ref("");
  const batchEntries = ref<BatchUrlEntry[]>([]);
  const batchSubmitProgress = ref<BatchSubmitProgress>({ done: 0, total: 0 });

  function setMessage(message: string) {
    notify.notifyInfo(message);
  }

  function setError(message: string) {
    notify.notifyError(message);
  }

  function clearMessage() {
    notify.clearAll();
  }

  function resetForm(): void {
    form.value.kind = "http";
    form.value.url = "";
    form.value.destinationDir = "";
    form.value.fileName = "";
    form.value.userAgent = "";
    form.value.threadMode = "adaptive";
    form.value.threadCount = DEFAULT_THREAD_COUNT;
    form.value.maxRetries = 5;
    form.value.checksum = "blake3";
    form.value.expectedChecksum = "";
    form.value.isProbingChecksum = false;
    form.value.checksumDetected = false;
    form.value.downloadLimitBps = null;
    form.value.uploadLimitBps = null;
    isFileNameLocked.value = false;
    autoDetectFileName.value = true;
  }

  function clampThreadCount(value: number): number {
    return Math.max(1, Math.min(MAX_THREADS, Math.trunc(value)));
  }

  function applySchedulerDefaults(mode: SchedulerMode, maxThreadsPerTask?: number) {
    if (form.value.kind !== "http") return;

    if (mode === "automatic") {
      form.value.threadMode = "adaptive";
      if (
        typeof maxThreadsPerTask === "number" &&
        Number.isFinite(maxThreadsPerTask) &&
        form.value.threadCount &&
        form.value.threadCount > maxThreadsPerTask
      ) {
        form.value.threadCount = maxThreadsPerTask;
      }
      return;
    }

    form.value.threadMode = "fixed";
    if (
      typeof maxThreadsPerTask === "number" &&
      Number.isFinite(maxThreadsPerTask) &&
      (!form.value.threadCount || form.value.threadCount > maxThreadsPerTask)
    ) {
      form.value.threadCount = maxThreadsPerTask;
      return;
    }

    if (!form.value.threadCount) {
      form.value.threadCount = 8;
    }
  }

  function applyAppSettingsDefaults(settings: AppSettings) {
    form.value.destinationDir = settings.download.defaultDownloadDir;
    form.value.maxRetries = settings.download.defaultMaxRetries;
    form.value.checksum = settings.download.defaultChecksum;
    form.value.userAgent = settings.download.defaultUserAgent;
    applySchedulerDefaults(settings.scheduler.mode, settings.scheduler.automatic.maxThreadsPerTask);
  }

  /** Apply the form-driven HTTP fields (thread mode/count, retries, checksum, name, UA). */
  function applyFormHttpFields(
    request: StartDownloadRequest,
    overrides?: { fileName?: string; userAgent?: string },
  ) {
    request.threadMode = form.value.threadMode;

    const fileName = (overrides?.fileName ?? form.value.fileName).trim();
    if (fileName) {
      request.fileName = fileName;
    }

    const userAgent = (overrides?.userAgent ?? form.value.userAgent).trim();
    if (userAgent) {
      request.userAgent = userAgent;
    }

    if (typeof form.value.threadCount === "number" && Number.isFinite(form.value.threadCount)) {
      request.threadCount = clampThreadCount(form.value.threadCount);
    }

    if (typeof form.value.maxRetries === "number" && Number.isFinite(form.value.maxRetries)) {
      const maxRetries = Math.trunc(form.value.maxRetries);
      if (maxRetries >= 0) {
        request.maxRetries = maxRetries;
      }
    }

    const expected = form.value.expectedChecksum.trim();
    if (expected) {
      request.expectedChecksum = expected;
      request.checksum = "sha256";
    } else {
      request.checksum = form.value.checksum;
    }
  }

  function buildStartRequest(): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: form.value.kind,
      url: form.value.url.trim(),
      destinationDir: form.value.destinationDir.trim(),
      userAgent: null,
      startPaused: false,
    };

    if (form.value.kind === "http") {
      applyFormHttpFields(request);
    } else if (form.value.kind === "bt") {
      if (form.value.selectedFileIndices && form.value.selectedFileIndices.length > 0) {
        request.selectedFileIndices = [...form.value.selectedFileIndices];
      }
      if (form.value.startPaused === true) {
        request.startPaused = true;
      }
    }

    return request;
  }

  async function pickDestinationDirectory() {
    if (isPickingDirectory.value) return;

    isPickingDirectory.value = true;

    try {
      const selectedPath = await pickDirectory();
      if (selectedPath) {
        form.value.destinationDir = selectedPath;
      }
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isPickingDirectory.value = false;
    }
  }

  async function pickTorrentSourceFile() {
    if (isPickingTorrent.value) return;

    isPickingTorrent.value = true;

    try {
      const selectedPath = await pickTorrentFile();
      if (selectedPath) {
        form.value.kind = "bt";
        form.value.url = selectedPath;
      }
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isPickingTorrent.value = false;
    }
  }

  async function submitStart(onSuccess?: (taskId: string) => Promise<void>) {
    if (isStarting.value) return;

    if (!form.value.url.trim() || !form.value.destinationDir.trim()) {
      setError(
        form.value.kind === "bt" ? t("messages.torrentStartRequired") : t("messages.startRequired"),
      );
      return;
    }

    isStarting.value = true;

    try {
      clearMessage();

      const taskId = await startDownload(buildStartRequest());
      const rawId = typeof taskId === "string" ? taskId : (taskId?.id ?? "mock-id");

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

      if (onSuccess) {
        await onSuccess(rawId);
      }

      setMessage(t("messages.downloadQueued", { id: rawId }));
      resetForm();
    } catch (error) {
      setError(toMessage(error));
    } finally {
      isStarting.value = false;
    }
  }

  async function autoFillFromClipboard(): Promise<void> {
    let text: string | null = null;
    try {
      text = await readClipboardText();
    } catch {
      return;
    }
    if (!text) return;

    const trimmed = text.trim();

    if (trimmed.startsWith("magnet:?")) {
      form.value.kind = "bt";
      form.value.url = trimmed;
    } else if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      form.value.kind = "http";
      form.value.url = trimmed;
    }
  }

  function parseBatchUrls(): void {
    const lines = batchUrls.value.split(/\r?\n/);
    const result: BatchUrlEntry[] = [];
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const expanded = expandUrlRanges(trimmed);
      for (const url of expanded) {
        const kind = detectKindFromUrl(url);
        const fileName = extractFileNameFromUrl(url);
        result.push({ id: crypto.randomUUID(), url, kind, fileName, status: "ready" });
      }
    }
    batchEntries.value = result;
  }

  function buildBatchRequest(entry: BatchUrlEntry): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: entry.kind,
      url: entry.url,
      destinationDir: form.value.destinationDir.trim(),
      userAgent: null,
      startPaused: false,
    };
    if (entry.kind === "http") {
      applyFormHttpFields(request, { fileName: entry.fileName });
    }
    return request;
  }

  async function submitBatch(onRefresh?: () => Promise<void>): Promise<void> {
    if (isStarting.value) return;
    if (!form.value.destinationDir.trim()) {
      setError(t("messages.startRequired"));
      return;
    }
    if (batchEntries.value.length === 0) {
      setError(t("composer.batchEmpty"));
      return;
    }

    isStarting.value = true;
    batchSubmitProgress.value = { done: 0, total: batchEntries.value.length };
    clearMessage();

    try {
      let successCount = 0;
      const errors: string[] = [];
      let completedCount = 0;

      const results = await Promise.all(
        batchEntries.value.map(async (entry) => {
          try {
            entry.status = "queued";
            await startDownload(buildBatchRequest(entry));
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

      if (onRefresh) {
        await onRefresh();
      }

      if (errors.length > 0) {
        setError(
          t("composer.batchCompletedWithErrors", {
            success: successCount,
            total: batchEntries.value.length,
            errors: errors.join("; "),
          }),
        );
      } else {
        setMessage(
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

  async function probeSha256Checksum() {
    const url = form.value.url.trim();
    if (!url || form.value.kind !== "http") return;
    if (form.value.expectedChecksum.trim() && !form.value.checksumDetected) return;

    form.value.isProbingChecksum = true;
    try {
      const fileName = form.value.fileName.trim() || undefined;
      const detected = await probeChecksum(url, fileName);
      if (detected) {
        form.value.expectedChecksum = detected;
        form.value.checksum = "sha256";
        form.value.checksumDetected = true;
      }
    } catch (err) {
      console.warn("Checksum probe failed", err);
    } finally {
      form.value.isProbingChecksum = false;
    }
  }

  function toggleBatchMode(): void {
    batchMode.value = !batchMode.value;
    if (!batchMode.value) {
      batchUrls.value = "";
      batchEntries.value = [];
      batchSubmitProgress.value = { done: 0, total: 0 };
    }
  }

  return {
    form,
    isPickingDirectory,
    isPickingTorrent,
    isFileNameLocked,
    autoDetectFileName,
    isStarting,
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
    resetForm,
    clampThreadCount,
    applySchedulerDefaults,
    applyAppSettingsDefaults,
    applyFormHttpFields,
    buildStartRequest,
    pickDestinationDirectory,
    pickTorrentSourceFile,
    submitStart,
    autoFillFromClipboard,
    parseBatchUrls,
    buildBatchRequest,
    submitBatch,
    toggleBatchMode,
    probeSha256Checksum,
  };
});
