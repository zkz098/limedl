import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// ── Mock Tauri core ─────────────────────────────────────────────────────────
vi.mock("#invoke", () => ({ invoke: vi.fn() }));

// ── Mock Tauri events ───────────────────────────────────────────────────────
// Capture the handler so tests can simulate download-progress/download-updated.
let onProgress: ((payload: Record<string, unknown>) => void) | null = null;
let onUpdated: ((payload: Record<string, unknown>) => void) | null = null;

vi.mock("#event", () => ({
  listen: vi.fn((event: string, handler: (evt: { payload: unknown }) => void) => {
    if (event === "download-progress") {
      onProgress = (payload) => handler({ payload });
    }
    if (event === "download-updated") {
      onUpdated = (payload) => handler({ payload });
    }
    return Promise.resolve(() => {});
  }),
}));

// ── Mock OS notifications ───────────────────────────────────────────────────
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
  onAction: vi.fn().mockResolvedValue({ unregister: vi.fn() }),
}));

// ── Mock useNotificationStore ───────────────────────────────────────────────
const { mockNotifySuccess, mockNotifyError, mockNotifyInfo, mockNotifyWarning } = vi.hoisted(
  () => ({
    mockNotifySuccess: vi.fn(),
    mockNotifyError: vi.fn(),
    mockNotifyInfo: vi.fn(),
    mockNotifyWarning: vi.fn(),
  }),
);

vi.mock("../../stores/notification", () => ({
  useNotificationStore: () => ({
    notifySuccess: mockNotifySuccess,
    notifyError: mockNotifyError,
    notifyInfo: mockNotifyInfo,
    notifyWarning: mockNotifyWarning,
    clearAll: vi.fn(),
    notify: vi.fn(),
    dismiss: vi.fn(),
    notifications: { value: [] },
  }),
}));

// ── Mock i18n ───────────────────────────────────────────────────────────────
vi.mock("../../i18n", () => ({
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) {
      return `${key} ${JSON.stringify(options)}`;
    }
    return key;
  }),
}));

// ── Mock download API ───────────────────────────────────────────────────────
vi.mock("../../lib/tauri/download-api", () => ({
  startDownload: vi.fn(),
  listDownloads: vi.fn(),
  getDownloadStatus: vi.fn(),
  getBtRuntimeStatus: vi.fn().mockResolvedValue({}),
  pauseDownload: vi.fn(),
  resumeDownload: vi.fn(),
  cancelDownload: vi.fn(),
  removeDownload: vi.fn(),
  purgeDownload: vi.fn(),
  openDownloadInExplorer: vi.fn(),
}));

// ── Imports (all after vi.mock) ─────────────────────────────────────────────
import { resetTauriMocks } from "../mocks/tauri-mock";
import {
  createMockDownloadTask,
  createMockDownloadList,
  createMockDownloadSnapshot,
  resetMockIds,
} from "../fixtures/downloads";
import type { BtRuntimeStatus, DownloadSummary } from "../../types/download";
import {
  startDownload,
  listDownloads,
  getDownloadStatus,
  getBtRuntimeStatus,
  pauseDownload,
  resumeDownload,
} from "../../lib/tauri/download-api";

const mockStartDownload = vi.mocked(startDownload);
const mockListDownloads = vi.mocked(listDownloads);
const mockGetDownloadStatus = vi.mocked(getDownloadStatus);
const mockPauseDownload = vi.mocked(pauseDownload);
const mockResumeDownload = vi.mocked(resumeDownload);

import { isPermissionGranted, sendNotification } from "@tauri-apps/plugin-notification";
import { removeDownload } from "../../lib/tauri/download-api";
const mockRemoveDownload = vi.mocked(removeDownload);
const mockIsPermissionGranted = vi.mocked(isPermissionGranted);
const mockSendNotification = vi.mocked(sendNotification);

// Pinia store under test
import { useDownloadStore } from "../../stores/download";

// ── Suite ───────────────────────────────────────────────────────────────────

describe("useDownloadStore", () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let store: ReturnType<typeof useDownloadStore>;

  beforeEach(async () => {
    vi.resetModules();
    resetTauriMocks();
    resetMockIds();
    onProgress = null;
    onUpdated = null;

    // Fresh Pinia instance for each test
    setActivePinia(createPinia());
    store = useDownloadStore();
  });

  afterEach(() => {
    store.destroyStore();
    vi.clearAllMocks();
  });

  // ── Initialization ──────────────────────────────────────────────────────

  describe("initialization", () => {
    it("creates with empty downloads", () => {
      expect(store.downloads).toEqual([]);
    });

    it("creates with null selectedId", () => {
      expect(store.selectedId).toBeNull();
    });
  });

  // ── upsertSummary (exercised via refreshStatus) ─────────────────────────

  describe("upsertSummary", () => {
    it("adds new download to empty list", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.refreshStatus("task-1", { silent: true });

      expect(store.downloads).toHaveLength(1);
      expect(store.downloads[0].id).toBe("task-1");
      expect(store.downloads[0].fileName).toBe("test.zip");
    });

    it("adds new download to front of non-empty list", async () => {
      // Prime with an existing download
      const existingSnap = createMockDownloadSnapshot({
        id: "existing-1",
        fileName: "old.zip",
      });
      mockGetDownloadStatus.mockResolvedValueOnce(existingSnap);
      await store.refreshStatus("existing-1", { silent: true });
      expect(store.downloads).toHaveLength(1);

      // New download should land at index 0 (unshift)
      const newSnap = createMockDownloadSnapshot({
        id: "new-1",
        fileName: "new.zip",
      });
      mockGetDownloadStatus.mockResolvedValueOnce(newSnap);
      await store.refreshStatus("new-1", { silent: true });

      expect(store.downloads).toHaveLength(2);
      expect(store.downloads[0].id).toBe("new-1");
      expect(store.downloads[1].id).toBe("existing-1");
    });

    it("updates existing download in-place", async () => {
      const snap1 = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        downloadedBytes: 100,
      });
      mockGetDownloadStatus.mockResolvedValueOnce(snap1);
      await store.refreshStatus("task-1", { silent: true });
      expect(store.downloads[0].downloadedBytes).toBe(100);

      // Same id — in-place update
      const snap2 = createMockDownloadSnapshot({
        id: "task-1",
        state: "completed",
        downloadedBytes: 500,
        totalBytes: 500,
      });
      mockGetDownloadStatus.mockResolvedValueOnce(snap2);
      await store.refreshStatus("task-1", { silent: true });

      expect(store.downloads).toHaveLength(1);
      expect(store.downloads[0].state).toBe("completed");
      expect(store.downloads[0].downloadedBytes).toBe(500);
    });
  });

  // ── patchProgress (exercised via captured event listener) ────────────────

  describe("patchProgress", () => {
    beforeEach(async () => {
      // Set up a download in the list via initStore path
      const task = createMockDownloadTask({
        id: "task-1",
        state: "queued",
        downloadedBytes: 0,
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      // Fire initStore to start listeners + refresh list
      await store.initStore();

      // Wait for listen promises to resolve and handlers to be captured
      await vi.waitFor(() => {
        expect(onProgress).not.toBeNull();
      });
    });

    it("updates downloading fields on existing download", () => {
      onProgress!({
        id: "task-1",
        state: "downloading",
        downloadedBytes: 2048,
        speedBytesPerSecond: 1_048_576,
        connectionCount: 4,
      });

      const entry = store.downloads.find((d: DownloadSummary) => d.id === "task-1");
      expect(entry!.state).toBe("downloading");
      expect(entry!.downloadedBytes).toBe(2048);
      expect(entry!.speedBytesPerSecond).toBe(1_048_576);
      expect(entry!.connectionCount).toBe(4);
    });

    it("does nothing for non-existent download id", () => {
      expect(store.downloads).toHaveLength(1);
      const originalBytes = store.downloads[0].downloadedBytes;

      onProgress!({
        id: "non-existent",
        state: "downloading",
        downloadedBytes: 9999,
        connectionCount: 2,
      });

      expect(store.downloads).toHaveLength(1);
      expect(store.downloads[0].downloadedBytes).toBe(originalBytes);
    });

    it("patches selectedSnapshot when id matches", async () => {
      // selectDownload sets selectedId and refreshStatus sets selectedSnapshot
      await store.selectDownload("task-1");
      await vi.waitFor(() => {
        expect(store.selectedSnapshot).not.toBeNull();
      });

      onProgress!({
        id: "task-1",
        state: "downloading",
        downloadedBytes: 5000,
        speedBytesPerSecond: 2_097_152,
        connectionCount: 6,
      });

      expect(store.selectedSnapshot!.downloadedBytes).toBe(5000);
      expect(store.selectedSnapshot!.state).toBe("downloading");
    });
  });

  // ── Form composition ────────────────────────────────────────────────────

  describe("form composition", () => {
    it("submitForm starts a download", async () => {
      mockStartDownload.mockResolvedValue({ kind: "http", id: "download-1" });
      mockListDownloads.mockResolvedValue([]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "download-1" }));

      store.form.url = "https://example.com/file.zip";
      store.form.destinationDir = "C:\\Downloads";
      store.form.fileName = "test.zip";

      await store.submitStart();

      expect(mockStartDownload).toHaveBeenCalledWith(
        expect.objectContaining({
          url: "https://example.com/file.zip",
          destinationDir: "C:\\Downloads",
          fileName: "test.zip",
        }),
      );
    });

    it("resetForm clears form fields after submit", async () => {
      mockStartDownload.mockResolvedValue({ kind: "http", id: "download-2" });
      mockListDownloads.mockResolvedValue([]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "download-2" }));

      store.form.url = "https://example.com/another.zip";
      store.form.destinationDir = "C:\\Downloads";
      store.form.fileName = "custom.zip";
      store.form.userAgent = "TestAgent/1.0";
      store.form.threadMode = "fixed";
      store.form.threadCount = 4;

      await store.submitStart();

      expect(store.form.url).toBe("");
      expect(store.form.destinationDir).toBe("");
      expect(store.form.fileName).toBe("");
      expect(store.form.userAgent).toBe("");
      expect(store.form.threadMode).toBe("adaptive");
      expect(store.form.threadCount).toBe(8);
    });
  });

  // ── Action composition ──────────────────────────────────────────────────

  describe("action composition", () => {
    beforeEach(async () => {
      const snap = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockGetDownloadStatus.mockResolvedValue(snap);
      await store.refreshStatus("task-1", { silent: true });
      await store.selectDownload("task-1");
    });

    it("pause pauses the selected download", async () => {
      const pausedSnap = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(pausedSnap);

      await store.runPause();

      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
    });

    it("resume resumes selected download", async () => {
      const resumedSnap = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockResumeDownload.mockResolvedValue(resumedSnap);

      await store.runResume();

      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── List operations ─────────────────────────────────────────────────────

  describe("list operations", () => {
    it("refreshList fetches and updates downloads", async () => {
      const tasks = createMockDownloadList(2, { fileName: "test.zip" });
      mockListDownloads.mockResolvedValue(tasks);

      await store.refreshList();

      expect(mockListDownloads).toHaveBeenCalled();
      expect(store.downloads).toHaveLength(2);
      expect(store.downloads[0].fileName).toBe("test.zip");
    });

    it("refreshStatus fetches single download status", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "status-1",
        fileName: "status.zip",
      });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.refreshStatus("status-1", { silent: true });

      expect(mockGetDownloadStatus).toHaveBeenCalledWith("status-1");
      expect(store.downloads).toHaveLength(1);
      expect(store.downloads[0].fileName).toBe("status.zip");
    });
  });

  // ── Notification ────────────────────────────────────────────────────────

  describe("notification", () => {
    it("download failed triggers onDownloadFailed callback", async () => {
      const onDownloadFailed = vi.fn();

      store.configure({ onDownloadFailed });

      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      await store.initStore();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      const failedSummary = createMockDownloadTask({
        id: "task-1",
        state: "failed",
        fileName: "test.zip",
        error: "Connection reset by peer",
      });
      onUpdated!(JSON.parse(JSON.stringify(failedSummary)));

      expect(onDownloadFailed).toHaveBeenCalledTimes(1);
      expect(onDownloadFailed).toHaveBeenCalledWith("test.zip", expect.any(String));
    });
  });

  // ── removeSummary ──────────────────────────────────────────────────────

  describe("removeSummary", () => {
    beforeEach(async () => {
      const snap1 = createMockDownloadSnapshot({ id: "task-1", fileName: "a.zip" });
      const snap2 = createMockDownloadSnapshot({ id: "task-2", fileName: "b.zip" });
      mockGetDownloadStatus.mockResolvedValueOnce(snap1);
      mockGetDownloadStatus.mockResolvedValueOnce(snap2);
      await store.refreshStatus("task-1", { silent: true });
      await store.refreshStatus("task-2", { silent: true });
    });

    it("clears selection when removing currently-selected download", async () => {
      await store.selectDownload("task-1");
      mockRemoveDownload.mockResolvedValue(
        createMockDownloadSnapshot({ id: "task-1", fileName: "a.zip" }),
      );

      await store.runDeleteTask("task-1");

      expect(store.downloads).toHaveLength(1);
      expect(store.downloads[0].id).toBe("task-2");
      expect(store.selectedId).toBeNull();
      expect(store.selectedSnapshot).toBeNull();
    });

    it("preserves selection when removing non-selected download", async () => {
      await store.selectDownload("task-1");
      mockRemoveDownload.mockResolvedValue(
        createMockDownloadSnapshot({ id: "task-2", fileName: "b.zip" }),
      );

      await store.runDeleteTask("task-2");

      expect(store.downloads).toHaveLength(1);
      expect(store.selectedId).toBe("task-1");
    });

    it("calls onDownloadsRemoved callback when provided", async () => {
      const onDownloadsRemoved = vi.fn();
      store.configure({ onDownloadsRemoved });

      const snap = createMockDownloadSnapshot({ id: "task-1", fileName: "a.zip" });
      mockGetDownloadStatus.mockResolvedValueOnce(snap);
      await store.refreshStatus("task-1", { silent: true });

      mockRemoveDownload.mockResolvedValue(
        createMockDownloadSnapshot({ id: "task-1", fileName: "a.zip" }),
      );

      await store.runDeleteTask("task-1");

      expect(onDownloadsRemoved).toHaveBeenCalledTimes(1);
      expect(onDownloadsRemoved).toHaveBeenCalledWith(["task-1"]);
    });
  });

  // ── ensureSelection ─────────────────────────────────────────────────────

  describe("ensureSelection", () => {
    it("auto-selects first download when allowAutoSelect=true and no current selection", async () => {
      const tasks = createMockDownloadList(2, { fileName: "test.zip" });
      mockListDownloads.mockResolvedValue(tasks);

      await store.refreshList();

      expect(store.selectedId).toBe(tasks[0].id);
    });

    it("clears selection when allowAutoSelect=false", async () => {
      const tasks = createMockDownloadList(2, { fileName: "test.zip" });
      mockListDownloads.mockResolvedValue(tasks);
      await store.refreshList();
      expect(store.selectedId).toBe(tasks[0].id);

      await store.selectDownload(null);
      expect(store.selectedId).toBeNull();

      const newTasks = createMockDownloadList(1, { fileName: "b.zip" });
      mockListDownloads.mockResolvedValue(newTasks);
      await store.refreshList();

      expect(store.selectedId).toBeNull();
    });

    it("no-op when selected ID is still in the list", async () => {
      const tasks = createMockDownloadList(1, { fileName: "test.zip", id: "stay-id" });
      mockListDownloads.mockResolvedValue(tasks);
      await store.refreshList();

      expect(store.selectedId).toBe("stay-id");

      mockListDownloads.mockResolvedValue(tasks);
      await store.refreshList();

      expect(store.selectedId).toBe("stay-id");
    });
  });

  // ── refreshBtRuntimeStatus ──────────────────────────────────────────────

  describe("refreshBtRuntimeStatus", () => {
    it("success path updates btRuntimeStatus", async () => {
      const status: BtRuntimeStatus = {
        connected: true,
        dhtEnabled: true,
        dhtNodes: 42,
        torrentCount: 5,
        peerCount: 100,
        uploadSpeedBytesPerSecond: 1_000_000,
        uploadedBytes: 0,
        updatedAtMs: Date.now(),
        seedCount: null,
        leechCount: null,
      };
      vi.mocked(getBtRuntimeStatus).mockResolvedValue(status);

      await store.refreshBtRuntimeStatus();

      expect(store.btRuntimeStatus).toEqual(status);
      expect(mockNotifyError).not.toHaveBeenCalled();
    });

    it("silent error does NOT show error toast", async () => {
      vi.mocked(getBtRuntimeStatus).mockRejectedValue(new Error("Network unavailable"));

      await store.refreshBtRuntimeStatus({ silent: true });

      expect(mockNotifyError).not.toHaveBeenCalled();
    });

    it("non-silent error DOES show error toast", async () => {
      vi.mocked(getBtRuntimeStatus).mockRejectedValue(new Error("Connection timeout"));

      await store.refreshBtRuntimeStatus();

      expect(mockNotifyError).toHaveBeenCalledTimes(1);
      expect(mockNotifyError).toHaveBeenCalledWith("errors.connectionTimeout");
    });
  });

  // ── handleDownloadUpdated ───────────────────────────────────────────────

  describe("handleDownloadUpdated", () => {
    it("downloading → completed shows success toast", async () => {
      const onDownloadFailed = vi.fn();
      store.configure({ onDownloadFailed });

      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      await store.initStore();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      const completed = createMockDownloadTask({
        id: "task-1",
        state: "completed",
        fileName: "test.zip",
      });
      onUpdated!(JSON.parse(JSON.stringify(completed)));

      expect(mockNotifySuccess).toHaveBeenCalledWith("notifications.downloadComplete");
      expect(onDownloadFailed).not.toHaveBeenCalled();
    });

    it("downloading → failed calls onDownloadFailed callback with filename and error", async () => {
      const onDownloadFailed = vi.fn();
      store.configure({ onDownloadFailed });

      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      await store.initStore();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      const failedSummary = createMockDownloadTask({
        id: "task-1",
        state: "failed",
        fileName: "test.zip",
        error: "HTTP status 404",
      });
      onUpdated!(JSON.parse(JSON.stringify(failedSummary)));

      expect(onDownloadFailed).toHaveBeenCalledTimes(1);
      expect(onDownloadFailed).toHaveBeenCalledWith(
        "test.zip",
        expect.stringContaining("errors.http404"),
      );
      expect(mockNotifySuccess).not.toHaveBeenCalled();
    });

    it("same-state transition does not fire notifications", async () => {
      const onDownloadFailed = vi.fn();
      store.configure({ onDownloadFailed });

      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      await store.initStore();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      const sameState = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
        downloadedBytes: 500,
      });
      onUpdated!(JSON.parse(JSON.stringify(sameState)));

      expect(onDownloadFailed).not.toHaveBeenCalled();
      expect(mockNotifySuccess).not.toHaveBeenCalled();
    });

    it("sends OS notification when enabled and download completes", async () => {
      mockIsPermissionGranted.mockResolvedValue(true);

      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(createMockDownloadSnapshot({ id: "task-1" }));

      await store.initStore();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      store.setNotificationsEnabled(true);

      const completed = createMockDownloadTask({
        id: "task-1",
        state: "completed",
        fileName: "test.zip",
      });
      onUpdated!(JSON.parse(JSON.stringify(completed)));

      await vi.waitFor(() => {
        expect(mockSendNotification).toHaveBeenCalledWith({
          title: "notifications.downloadComplete",
          body: expect.stringContaining("test.zip"),
          extra: { downloadId: "task-1" },
        });
      });
    });
  });

  // ── setNotificationsEnabled ─────────────────────────────────────────────

  describe("setNotificationsEnabled", () => {
    it("toggles the notifications flag without error", () => {
      expect(typeof store.setNotificationsEnabled).toBe("function");

      store.setNotificationsEnabled(true);
      store.setNotificationsEnabled(false);
    });
  });
});
