import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ── Mock Tauri core ─────────────────────────────────────────────────────────
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// ── Mock Vue lifecycle hooks ────────────────────────────────────────────────
// Capture onMounted/onUnmounted callbacks so tests can trigger them manually.
let capturedOnMounted: (() => Promise<void>) | null = null;
let capturedOnUnmounted: (() => void) | null = null;

vi.mock("vue", async () => {
  const actual = await vi.importActual<typeof import("vue")>("vue");
  return {
    ...actual,
    onMounted: vi.fn((cb: () => void) => {
      capturedOnMounted = cb as () => Promise<void>;
    }),
    onUnmounted: vi.fn((cb: () => void) => {
      capturedOnUnmounted = cb;
    }),
  };
});

// ── Mock Tauri events ───────────────────────────────────────────────────────
// Capture the handler so tests can simulate download-progress/download-updated.
let onProgress: ((payload: Record<string, unknown>) => void) | null = null;
let onUpdated: ((payload: Record<string, unknown>) => void) | null = null;

vi.mock("@tauri-apps/api/event", () => ({
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
import type { DownloadSummary } from "../../types/download";
import {
  startDownload,
  listDownloads,
  getDownloadStatus,
  pauseDownload,
  resumeDownload,
} from "../../lib/tauri/download-api";

const mockStartDownload = vi.mocked(startDownload);
const mockListDownloads = vi.mocked(listDownloads);
const mockGetDownloadStatus = vi.mocked(getDownloadStatus);
const mockPauseDownload = vi.mocked(pauseDownload);
const mockResumeDownload = vi.mocked(resumeDownload);

// ── Suite ───────────────────────────────────────────────────────────────────

describe("useDownloader", () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let downloader: any;

  beforeEach(async () => {
    vi.resetModules();
    resetTauriMocks();
    resetMockIds();
    capturedOnMounted = null;
    capturedOnUnmounted = null;
    onProgress = null;
    onUpdated = null;

    // Dynamically import to get a fresh module (clears the singleton guard)
    const mod = await import("../../composables/useDownloader");
    downloader = mod.createDownloader();
  });

  afterEach(() => {
    capturedOnUnmounted?.();
    vi.clearAllMocks();
  });

  // ── Initialization ──────────────────────────────────────────────────────

  describe("initialization", () => {
    it("creates with empty downloads", () => {
      expect(downloader.downloads.value).toEqual([]);
    });

    it("creates with null selectedId", () => {
      expect(downloader.selectedId.value).toBeNull();
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

      await downloader.refreshStatus("task-1", { silent: true });

      expect(downloader.downloads.value).toHaveLength(1);
      expect(downloader.downloads.value[0].id).toBe("task-1");
      expect(downloader.downloads.value[0].fileName).toBe("test.zip");
    });

    it("adds new download to front of non-empty list", async () => {
      // Prime with an existing download
      const existingSnap = createMockDownloadSnapshot({
        id: "existing-1",
        fileName: "old.zip",
      });
      mockGetDownloadStatus.mockResolvedValueOnce(existingSnap);
      await downloader.refreshStatus("existing-1", { silent: true });
      expect(downloader.downloads.value).toHaveLength(1);

      // New download should land at index 0 (unshift)
      const newSnap = createMockDownloadSnapshot({
        id: "new-1",
        fileName: "new.zip",
      });
      mockGetDownloadStatus.mockResolvedValueOnce(newSnap);
      await downloader.refreshStatus("new-1", { silent: true });

      expect(downloader.downloads.value).toHaveLength(2);
      expect(downloader.downloads.value[0].id).toBe("new-1");
      expect(downloader.downloads.value[1].id).toBe("existing-1");
    });

    it("updates existing download in-place", async () => {
      const snap1 = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        downloadedBytes: 100,
      });
      mockGetDownloadStatus.mockResolvedValueOnce(snap1);
      await downloader.refreshStatus("task-1", { silent: true });
      expect(downloader.downloads.value[0].downloadedBytes).toBe(100);

      // Same id — in-place update
      const snap2 = createMockDownloadSnapshot({
        id: "task-1",
        state: "completed",
        downloadedBytes: 500,
        totalBytes: 500,
      });
      mockGetDownloadStatus.mockResolvedValueOnce(snap2);
      await downloader.refreshStatus("task-1", { silent: true });

      expect(downloader.downloads.value).toHaveLength(1);
      expect(downloader.downloads.value[0].state).toBe("completed");
      expect(downloader.downloads.value[0].downloadedBytes).toBe(500);
    });
  });

  // ── patchProgress (exercised via captured event listener) ────────────────

  describe("patchProgress", () => {
    beforeEach(async () => {
      // Set up a download in the list via the onMounted path
      const task = createMockDownloadTask({
        id: "task-1",
        state: "queued",
        downloadedBytes: 0,
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(
        createMockDownloadSnapshot({ id: "task-1" }),
      );

      // Fire onMounted to start listeners + refresh list
      await capturedOnMounted!();

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

      const entry = downloader.downloads.value.find(
        (d: DownloadSummary) => d.id === "task-1",
      );
      expect(entry.state).toBe("downloading");
      expect(entry.downloadedBytes).toBe(2048);
      expect(entry.speedBytesPerSecond).toBe(1_048_576);
      expect(entry.connectionCount).toBe(4);
    });

    it("does nothing for non-existent download id", () => {
      expect(downloader.downloads.value).toHaveLength(1);
      const originalBytes = downloader.downloads.value[0].downloadedBytes;

      onProgress!({
        id: "non-existent",
        state: "downloading",
        downloadedBytes: 9999,
        connectionCount: 2,
      });

      expect(downloader.downloads.value).toHaveLength(1);
      expect(
        downloader.downloads.value[0].downloadedBytes,
      ).toBe(originalBytes);
    });

    it("patches selectedSnapshot when id matches", async () => {
      // selectDownload sets selectedId and refreshStatus sets selectedSnapshot
      await downloader.selectDownload("task-1");
      await vi.waitFor(() => {
        expect(downloader.selectedSnapshot.value).not.toBeNull();
      });

      onProgress!({
        id: "task-1",
        state: "downloading",
        downloadedBytes: 5000,
        speedBytesPerSecond: 2_097_152,
        connectionCount: 6,
      });

      expect(downloader.selectedSnapshot.value!.downloadedBytes).toBe(5000);
      expect(downloader.selectedSnapshot.value!.state).toBe("downloading");
    });
  });

  // ── Form composition ────────────────────────────────────────────────────

  describe("form composition", () => {
    it("submitForm starts a download", async () => {
      mockStartDownload.mockResolvedValue("download-1");
      mockListDownloads.mockResolvedValue([]);
      mockGetDownloadStatus.mockResolvedValue(
        createMockDownloadSnapshot({ id: "download-1" }),
      );

      downloader.form.url = "https://example.com/file.zip";
      downloader.form.destinationDir = "C:\\Downloads";
      downloader.form.fileName = "test.zip";

      await downloader.submitStart();

      expect(mockStartDownload).toHaveBeenCalledWith(
        expect.objectContaining({
          url: "https://example.com/file.zip",
          destinationDir: "C:\\Downloads",
          fileName: "test.zip",
        }),
      );
    });

    it("resetForm clears form fields after submit", async () => {
      mockStartDownload.mockResolvedValue("download-2");
      mockListDownloads.mockResolvedValue([]);
      mockGetDownloadStatus.mockResolvedValue(
        createMockDownloadSnapshot({ id: "download-2" }),
      );

      downloader.form.url = "https://example.com/another.zip";
      downloader.form.destinationDir = "C:\\Downloads";
      downloader.form.fileName = "custom.zip";
      downloader.form.userAgent = "TestAgent/1.0";
      downloader.form.threadMode = "fixed";
      downloader.form.threadCount = 4;

      await downloader.submitStart();

      // After resetForm:
      expect(downloader.form.url).toBe("");
      expect(downloader.form.destinationDir).toBe("");
      expect(downloader.form.fileName).toBe("");
      expect(downloader.form.userAgent).toBe("");
      expect(downloader.form.threadMode).toBe("adaptive");
      expect(downloader.form.threadCount).toBe(8);
    });
  });

  // ── Action composition ──────────────────────────────────────────────────

  describe("action composition", () => {
    beforeEach(async () => {
      // Add a download to the list and select it
      const snap = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockGetDownloadStatus.mockResolvedValue(snap);
      await downloader.refreshStatus("task-1", { silent: true });
      await downloader.selectDownload("task-1");
    });

    it("pause pauses the selected download", async () => {
      const pausedSnap = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(pausedSnap);

      await downloader.runPause();

      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
    });

    it("resume resumes selected download", async () => {
      const resumedSnap = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockResumeDownload.mockResolvedValue(resumedSnap);

      await downloader.runResume();

      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── List operations ─────────────────────────────────────────────────────

  describe("list operations", () => {
    it("refreshList fetches and updates downloads", async () => {
      const tasks = createMockDownloadList(2, { fileName: "test.zip" });
      mockListDownloads.mockResolvedValue(tasks);

      await downloader.refreshList();

      expect(mockListDownloads).toHaveBeenCalled();
      expect(downloader.downloads.value).toHaveLength(2);
      expect(downloader.downloads.value[0].fileName).toBe("test.zip");
    });

    it("refreshStatus fetches single download status", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "status-1",
        fileName: "status.zip",
      });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await downloader.refreshStatus("status-1", { silent: true });

      expect(mockGetDownloadStatus).toHaveBeenCalledWith("status-1");
      expect(downloader.downloads.value).toHaveLength(1);
      expect(downloader.downloads.value[0].fileName).toBe("status.zip");
    });
  });

  // ── Notification ────────────────────────────────────────────────────────

  describe("notification", () => {
    it("download failed triggers onDownloadFailed callback", async () => {
      const onDownloadFailed = vi.fn();

      // Create a fresh downloader with the callback
      vi.resetModules();
      const mod = await import("../../composables/useDownloader");
      mod.createDownloader({ onDownloadFailed });

      // Populate the list and start listeners
      const task = createMockDownloadTask({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockListDownloads.mockResolvedValue([task]);
      mockGetDownloadStatus.mockResolvedValue(
        createMockDownloadSnapshot({ id: "task-1" }),
      );

      await capturedOnMounted!();
      await vi.waitFor(() => {
        expect(onUpdated).not.toBeNull();
      });

      // Send a failure update event
      const failedSummary = createMockDownloadTask({
        id: "task-1",
        state: "failed",
        fileName: "test.zip",
        error: "Connection reset by peer",
      });
      onUpdated!(failedSummary as unknown as Record<string, unknown>);

      expect(onDownloadFailed).toHaveBeenCalledTimes(1);
      expect(onDownloadFailed).toHaveBeenCalledWith(
        "test.zip",
        expect.any(String),
      );
    });
  });
});
