import { detectKindFromUrl, extractFileNameFromUrl } from "../lib/url-utils";
import { computed, reactive, ref, watch, type Ref } from "vue";

import { setBtSpeedLimit, startDownload } from "../lib/tauri/download-api";
import { pickDirectory, pickTorrentFile } from "../lib/tauri/dialog-api";
import { t } from "../i18n";
import { toMessage } from "./downloadHelpers";
import type {
  BatchUrlEntry,
  BatchSubmitProgress,
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

  // ── Form validation & auto-detection ──────────────────────────────

  const isFormValid = computed(() => form.url.trim() !== "" && form.destinationDir.trim() !== "");
  const isFileNameLocked = ref(false);
  const autoDetectFileName = ref(true);

  const DEFAULT_THREAD_COUNT = 8;
  const MAX_THREADS = 128;

  /**
   * Extracts a file name from a URL and sets form.fileName.
   * Does nothing when isFileNameLocked is true.
   */
  function autoSetFileName(url: string): void {
    if (isFileNameLocked.value) return;

    const trimmed = url.trim();
    if (!trimmed) {
      form.fileName = "";
      return;
    }

    // Magnet link — extract display name from dn parameter
    if (trimmed.toLowerCase().startsWith("magnet:")) {
      const queryIndex = trimmed.indexOf("?");
      const query = queryIndex >= 0 ? trimmed.slice(queryIndex + 1) : "";
      const dn = new URLSearchParams(query).get("dn");
      form.fileName = dn ? decodeURIComponent(dn) : "";
      return;
    }

    try {
      const parsed = new URL(trimmed);
      const segment = parsed.pathname.split("/").pop();
      form.fileName = segment ? decodeURIComponent(segment) : "";
    } catch {
      form.fileName = "";
    }
  }

  /** Reset form to initial defaults. */
  function resetForm(): void {
    form.kind = "http";
    form.url = "";
    form.destinationDir = "";
    form.fileName = "";
    form.userAgent = "";
    form.threadMode = "adaptive";
    form.threadCount = DEFAULT_THREAD_COUNT;
    form.maxRetries = 5;
    form.checksum = "blake3" as ChecksumMode;
    form.downloadLimitBps = null;
    form.uploadLimitBps = null;
    isFileNameLocked.value = false;
    autoDetectFileName.value = true;
  }

  /** Clamp thread count to valid range [1, MAX_THREADS]. */
  function clampThreadCount(value: number): number {
    return Math.max(1, Math.min(MAX_THREADS, Math.trunc(value)));
  }

  /** Validate and clamp form.threadCount in place. */
  function validateThreadCount(): void {
    if (typeof form.threadCount === "number" && Number.isFinite(form.threadCount)) {
      form.threadCount = clampThreadCount(form.threadCount);
    } else {
      form.threadCount = DEFAULT_THREAD_COUNT;
    }
  }

  // ── URL change auto-detect ───────────────────────────────────────

  watch(
    () => form.url,
    (newUrl) => {
      if (newUrl && autoDetectFileName.value) {
        autoSetFileName(newUrl);
      }
    },
  );

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
    // Per-download rate limits (downloadLimitBps / uploadLimitBps) are not yet
    // exposed as defaults in AppSettings. The form starts with null (no limit)
    // until settings-level per-download rate limit defaults are added.
  }

  function buildStartRequest(): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: form.kind,
      url: form.url.trim(),
      destinationDir: form.destinationDir.trim(),
      userAgent: null,
      startPaused: false,
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
        request.threadCount = clampThreadCount(form.threadCount);
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
    if (isStarting.value) return;

    if (!form.url.trim() || !form.destinationDir.trim()) {
      setError(
        form.kind === "bt" ? t("messages.torrentStartRequired") : t("messages.startRequired"),
      );
      return;
    }

    isStarting.value = true;

    try {
      clearMessage();

      const taskId = await startDownload(buildStartRequest());
      allowAutoSelect.value = true;
      selectedId.value = taskId.id;

      // For BT downloads, apply initial per-download speed limits after start
      // (the StartDownloadRequest no longer carries these fields).
      if (form.kind === "bt") {
        const dl = form.downloadLimitBps;
        const ul = form.uploadLimitBps;
        if ((dl !== null && dl > 0) || (ul !== null && ul > 0)) {
          try {
            await setBtSpeedLimit(taskId.id, dl ?? undefined, ul ?? undefined);
          } catch {
            // Non-critical — speed limit is optional, the download is already running.
          }
        }
      }

      await refreshList();
      await refreshStatus(taskId.id, { silent: true });
      setMessage(t("messages.downloadQueued", { id: taskId.id }));
      resetForm();
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
    // Use Tauri's native clipboard plugin when available (no permission
    // prompt) — falls back silently in NAS/browser mode.
    let text: string | null = null;
    try {
      const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
      text = await readText();
    } catch {
      return;
    }
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
  }

  // ── Batch import mode ──────────────────────────────────────────────
  const batchMode = ref(false);
  const batchUrls = ref("");
  const batchEntries = ref<BatchUrlEntry[]>([]);
  const batchSubmitProgress = ref<BatchSubmitProgress>({ done: 0, total: 0 });

  /** Expand URL range patterns like file[01-20].zip or file[1-20].zip */
  function expandUrlRanges(url: string): string[] {
    const rangeRegex = /\[(\d+)-(\d+)\]/;
    const match = rangeRegex.exec(url);
    if (!match) return [url];
    const start = parseInt(match[1], 10);
    const end = parseInt(match[2], 10);
    if (start > end) return [url]; // treat as literal text, not a range
    const padding = match[1].length;
    const results: string[] = [];
    for (let i = start; i <= end; i++) {
      results.push(url.replace(rangeRegex, String(i).padStart(padding, "0")));
    }
    return results;
  }

  /** Parse batch textarea content into BatchUrlEntry array. */
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

  /** Build a StartDownloadRequest for a batch entry using shared form settings */
  function buildBatchRequest(entry: BatchUrlEntry): StartDownloadRequest {
    const request: StartDownloadRequest = {
      kind: entry.kind,
      url: entry.url,
      destinationDir: form.destinationDir.trim(),
      userAgent: null,
      startPaused: false,
    };
    if (entry.kind === "http") {
      request.threadMode = form.threadMode;
      const fileName = entry.fileName.trim();
      if (fileName) request.fileName = fileName;
      const userAgent = form.userAgent.trim();
      if (userAgent) request.userAgent = userAgent;
      if (typeof form.threadCount === "number" && Number.isFinite(form.threadCount)) {
        request.threadCount = clampThreadCount(form.threadCount);
      }
      if (typeof form.maxRetries === "number" && Number.isFinite(form.maxRetries)) {
        const maxRetries = Math.trunc(form.maxRetries);
        if (maxRetries >= 0) request.maxRetries = maxRetries;
      }
      request.checksum = form.checksum;
    }
    return request;
  }

  async function submitBatch(): Promise<void> {
    if (isStarting.value) return;
    if (!form.destinationDir.trim()) {
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

      for (let i = 0; i < batchEntries.value.length; i++) {
        const entry = batchEntries.value[i];
        try {
          entry.status = "queued";
          await startDownload(buildBatchRequest(entry));
          entry.status = "success";
          successCount++;
        } catch (error) {
          entry.status = "error";
          entry.error = toMessage(error);
          errors.push(`${entry.fileName || entry.url}: ${toMessage(error)}`);
        }
        batchSubmitProgress.value = { done: i + 1, total: batchEntries.value.length };
      }

      await refreshList();
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

      // Keep failed entries visible so user can review and retry
      batchEntries.value = batchEntries.value.filter((e) => e.status === "error");
      // Clear textarea only if ALL succeeded
      if (batchEntries.value.length === 0) {
        batchUrls.value = "";
      }
    } finally {
      isStarting.value = false;
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
    isStarting,
    isPickingDirectory,
    isPickingTorrent,
    isFormValid,
    isFileNameLocked,
    autoDetectFileName,
    applySchedulerDefaults,
    applyAppSettingsDefaults,
    buildStartRequest,
    autoSetFileName,
    resetForm,
    clampThreadCount,
    validateThreadCount,
    pickDestinationDirectory,
    pickTorrentSourceFile,
    submitStart,
    autoFillFromClipboard,
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
    parseBatchUrls,
    submitBatch,
    toggleBatchMode,
  };
}
