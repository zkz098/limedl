import { computed, reactive, ref, watch, type Ref } from "vue";

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
  };
}
