import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "#event";
import {
  isPermissionGranted,
  onAction,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

import {
  cancelDownload,
  getBtRuntimeStatus,
  getDownloadStatus,
  listDownloads,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
  setBtSpeedLimit,
  setPriority,
  startDownload,
} from "../lib/tauri/download-api";
import { pickDirectory, pickTorrentFile } from "../lib/tauri/dialog-api";
import { t } from "../i18n";
import {
  canPauseState,
  canResumeState,
  terminalStates,
  toFriendlyError,
  toMessage,
  toSummary,
} from "../composables/downloadHelpers";
import { detectKindFromUrl, extractFileNameFromUrl } from "../lib/url-utils";
import { useNotificationStore } from "./notification";
import type { AppSettings, SchedulerMode } from "../types/settings";
import type {
  BatchSubmitProgress,
  BatchUrlEntry,
  BtRuntimeStatus,
  DownloadFormState,
  DownloadProgress,
  DownloadSnapshot,
  DownloadSummary,
  Priority,
  StartDownloadRequest,
} from "../types/download";

// ── Options interface ──────────────────────────────────────────────

export interface DownloadStoreOptions {
  /** Called when a download transitions to failed (for in-app notification) */
  onDownloadFailed?: (fileName: string, reason: string) => void;
  /** Called when one or more downloads are removed from the list */
  onDownloadsRemoved?: (removedIds: string[]) => void;
}

// ── OS notification (standalone, not part of store) ────────────────

async function fireNotification(title: string, body: string, downloadId?: string) {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const permission = await requestPermission();
      granted = permission === "granted";
    }
    if (granted) {
      sendNotification({
        title,
        body,
        ...(downloadId ? { extra: { downloadId } } : {}),
      });
    }
  } catch {
    // Silently fail — notifications are non-critical
  }
}

// ── URL range expansion ────────────────────────────────────────────

/** Expand URL range patterns like file[01-20].zip or file[1-20].zip */
function expandUrlRanges(url: string): string[] {
  const rangeRegex = /\[(\d+)-(\d+)\]/;
  const match = rangeRegex.exec(url);
  if (!match) return [url];
  const start = Number.parseInt(match[1], 10);
  const end = Number.parseInt(match[2], 10);
  if (start > end) return [url];
  const padding = match[1].length;
  const results: string[] = [];
  for (let i = start; i <= end; i++) {
    results.push(url.replace(rangeRegex, String(i).padStart(padding, "0")));
  }
  return results;
}

function canPauseDownload(download: DownloadSummary) {
  return canPauseState(download.state);
}

function canResumeDownload(download: DownloadSummary) {
  return canResumeState(download.state);
}

/** Copy the always-present progress fields and the optional non-null ones onto a task summary. */
function applyProgressToSummary(existing: DownloadSummary, progress: DownloadProgress) {
  existing.state = progress.state;
  existing.downloadedBytes = progress.downloadedBytes;
  existing.connectionCount = progress.connectionCount;
  if (progress.totalBytes != null) existing.totalBytes = progress.totalBytes;
  if (progress.speedBytesPerSecond != null)
    existing.speedBytesPerSecond = progress.speedBytesPerSecond;
  if (progress.etaSeconds != null) existing.etaSeconds = progress.etaSeconds;
  if (progress.allocatedThreadCount != null)
    existing.allocatedThreadCount = progress.allocatedThreadCount;
  if (progress.error != null) existing.error = progress.error;
  if (progress.uploadedBytes != null) existing.uploadedBytes = progress.uploadedBytes;
  if (progress.uploadSpeedBytesPerSecond != null)
    existing.uploadSpeedBytesPerSecond = progress.uploadSpeedBytesPerSecond;
  if (progress.peerCount != null) existing.peerCount = progress.peerCount;
  if (progress.uploadStatus != null) existing.uploadStatus = progress.uploadStatus;
  if (progress.degraded != null) existing.degraded = progress.degraded;
  if (progress.diskType != null) existing.diskType = progress.diskType;
  if (progress.flushing != null) existing.flushing = progress.flushing;
}

/** Mirror the live progress onto the detail side-panel snapshot (when selected). */
function applyProgressToSnapshot(snapshot: DownloadSnapshot, progress: DownloadProgress) {
  Object.assign(snapshot, {
    downloadedBytes: progress.downloadedBytes,
    state: progress.state,
    ...(progress.totalBytes != null && { totalBytes: progress.totalBytes }),
    ...(progress.speedBytesPerSecond != null && {
      speedBytesPerSecond: progress.speedBytesPerSecond,
    }),
    ...(progress.etaSeconds != null && { etaSeconds: progress.etaSeconds }),
    ...(progress.connectionCount !== undefined && {
      connectionCount: progress.connectionCount,
    }),
    ...(progress.error != null && { error: progress.error }),
    ...(progress.uploadedBytes != null && { uploadedBytes: progress.uploadedBytes }),
    ...(progress.uploadSpeedBytesPerSecond != null && {
      uploadSpeedBytesPerSecond: progress.uploadSpeedBytesPerSecond,
    }),
    ...(progress.peerCount != null && { peerCount: progress.peerCount }),
    ...(progress.uploadStatus != null && { uploadStatus: progress.uploadStatus }),
    ...(progress.degraded != null && { degraded: progress.degraded }),
    ...(progress.diskType != null && { diskType: progress.diskType }),
    ...(progress.flushing != null && { flushing: progress.flushing }),
  });
}

// ── Store ──────────────────────────────────────────────────────────

export const useDownloadStore = defineStore("download", () => {
  // ── Callbacks ────────────────────────────────────────────────────
  let callbacks: DownloadStoreOptions = {};
  function configure(opts: DownloadStoreOptions) {
    callbacks = opts;
  }

  // ── Notification store ───────────────────────────────────────────
  const notify = useNotificationStore();

  // ── Core reactive state ──────────────────────────────────────────
  const downloads = ref<DownloadSummary[]>([]);
  const selectedId = ref<string | null>(null);
  const selectedSnapshot = ref<DownloadSnapshot | null>(null);
  const isAutoRefreshing = ref(false);
  const btRuntimeStatus = ref<BtRuntimeStatus | null>(null);
  const isRefreshingStatus = ref(false);
  const allowAutoSelect = ref(true);
  const actionName = ref("");
  const isStarting = ref(false);
  const notificationsEnabled = ref(false);

  const isRefreshingList = ref(false);

  // ── Form state ───────────────────────────────────────────────────
  const form = ref<DownloadFormState>({
    kind: "http",
    url: "",
    destinationDir: "",
    fileName: "",
    userAgent: "",
    threadMode: "adaptive",
    threadCount: 8,
    maxRetries: 5,
    checksum: "blake3",
    downloadLimitBps: null,
    uploadLimitBps: null,
  });

  const isPickingDirectory = ref(false);
  const isPickingTorrent = ref(false);

  const isFileNameLocked = ref(false);
  const autoDetectFileName = ref(true);

  const DEFAULT_THREAD_COUNT = 8;
  const MAX_THREADS = 128;

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
  }

  function patchProgress(progress: DownloadProgress) {
    const existing = downloads.value.find((d) => d.id === progress.id);
    if (!existing) return;

    applyProgressToSummary(existing, progress);

    if (selectedId.value === progress.id && selectedSnapshot.value) {
      applyProgressToSnapshot(selectedSnapshot.value, progress);
    }
  }

  function removeSummary(downloadId: string) {
    downloads.value = downloads.value.filter((download) => download.id !== downloadId);

    callbacks.onDownloadsRemoved?.([downloadId]);

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

  // ── API helpers ─────────────────────────────────────────────────
  async function refreshBtRuntimeStatus(opts?: { silent?: boolean }) {
    try {
      btRuntimeStatus.value = await getBtRuntimeStatus();
    } catch (error) {
      if (!opts?.silent) {
        setError(toMessage(error));
      }
    }
  }

  async function refreshStatus(downloadId = selectedId.value, opts?: { silent?: boolean }) {
    if (!downloadId) return;

    if (isRefreshingStatus.value) return;

    isRefreshingStatus.value = true;

    try {
      const snapshot = await getDownloadStatus(downloadId);
      upsertSummary(toSummary(snapshot));

      if (selectedId.value === downloadId) {
        selectedSnapshot.value = snapshot;
      }

      if (!opts?.silent) {
        setMessage(t("messages.statusRefreshed", { fileName: snapshot.fileName }));
      }
    } catch (error) {
      if (!opts?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingStatus.value = false;
    }
  }

  // ── List operations ──────────────────────────────────────────────
  async function refreshList(options?: { silent?: boolean }) {
    if (isRefreshingList.value) return;

    isRefreshingList.value = true;

    try {
      const oldIds = new Set(downloads.value.map((d) => d.id));
      downloads.value = await listDownloads();
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

  // ── Form helpers ─────────────────────────────────────────────────
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

    request.checksum = form.value.checksum;
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

  async function submitStart() {
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
      allowAutoSelect.value = true;
      selectedId.value = taskId.id;

      if (form.value.kind === "bt") {
        const dl = form.value.downloadLimitBps;
        const ul = form.value.uploadLimitBps;
        if ((dl !== null && dl > 0) || (ul !== null && ul > 0)) {
          try {
            await setBtSpeedLimit(taskId.id, dl ?? undefined, ul ?? undefined);
          } catch {
            // Non-critical
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

  async function autoFillFromClipboard(): Promise<void> {
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
      form.value.kind = "bt";
      form.value.url = trimmed;
    } else if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      form.value.kind = "http";
      form.value.url = trimmed;
    }
  }

  // ── Batch import mode ────────────────────────────────────────────
  const batchMode = ref(false);
  const batchUrls = ref("");
  const batchEntries = ref<BatchUrlEntry[]>([]);
  const batchSubmitProgress = ref<BatchSubmitProgress>({ done: 0, total: 0 });

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

  async function submitBatch(): Promise<void> {
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

      batchEntries.value = batchEntries.value.filter((e) => e.status === "error");
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

  // ── Event handling ───────────────────────────────────────────────
  function handleDownloadUpdated(summary: DownloadSummary) {
    const existing = downloads.value.find((d) => d.id === summary.id);
    const oldState = existing?.state;

    if (oldState && oldState !== "failed" && summary.state === "failed") {
      callbacks.onDownloadFailed?.(
        summary.fileName,
        summary.error ? toFriendlyError(summary.error) : t("common.unknown"),
      );
    }

    if (oldState && oldState !== "completed" && summary.state === "completed") {
      notify.notifySuccess(t("notifications.downloadComplete"));
    }

    upsertSummary(summary);

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
      patchProgress(event.payload);
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
      ensureSelection();
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
          void listener.unregister();
          return;
        }
        unlistenNotificationAction = () => {
          void listener.unregister();
        };
      })
      .catch(() => {});

    btRuntimeTimer = setInterval(() => {
      void refreshBtRuntimeStatus({ silent: true });
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
    await refreshList({ silent: true });
    await refreshBtRuntimeStatus({ silent: true });

    if (selectedId.value) {
      await refreshStatus(selectedId.value, { silent: true });
    }

    startAutoRefresh();
  }

  function destroyStore() {
    stopAutoRefresh();
  }

  function setNotificationsEnabled(enabled: boolean) {
    notificationsEnabled.value = enabled;
  }

  // ── Return the store interface ──────────────────────────────────
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
    canPauseDownload,
    canResumeDownload,
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
    applySchedulerDefaults: applySchedulerDefaults,
    applyAppSettingsDefaults: applyAppSettingsDefaults,
    pickDestinationDirectory,
    pickTorrentSourceFile,

    // List
    refreshList,
    refreshBtRuntimeStatus,
    refreshStatus,

    // Actions
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

    // Selection
    selectDownload,
    selectedDownload,
    selectedId,
    selectedSnapshot,
    selectedSummary,

    // Form / submit
    submitStart,
    autoFillFromClipboard,

    // Batch
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
    parseBatchUrls,
    submitBatch,
    toggleBatchMode,
  };
});
