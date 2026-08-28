import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

import { setupDownloadStoreMocks } from "../fixtures/download-store-mocks";
setupDownloadStoreMocks();

import { resetTauriMocks } from "../mocks/tauri-mock";
import { useDownloadStore } from "../../stores/download/index";
import { createMockDownloadSnapshot, DownloadPresets, resetMockIds } from "../fixtures/downloads";

import {
  cancelDownload,
  getDownloadStatus,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
} from "../../lib/tauri/download-api";
import type { DownloadSnapshot } from "../../types/download";

const mockCancelDownload = vi.mocked(cancelDownload);
const mockGetDownloadStatus = vi.mocked(getDownloadStatus);
const mockOpenDownloadInExplorer = vi.mocked(openDownloadInExplorer);
const mockPauseDownload = vi.mocked(pauseDownload);
const mockPurgeDownload = vi.mocked(purgeDownload);
const mockRemoveDownload = vi.mocked(removeDownload);
const mockResumeDownload = vi.mocked(resumeDownload);

describe("useDownloadStore (actions)", () => {
  let store: ReturnType<typeof useDownloadStore>;

  beforeEach(() => {
    resetTauriMocks();
    resetMockIds();
    setActivePinia(createPinia());
    store = useDownloadStore();
  });

  afterEach(() => {
    vi.clearAllMocks();
    store.destroyStore();
  });

  // ── selectDownload ──────────────────────────────────────────────────────

  describe("selectDownload", () => {
    it("sets selectedId and calls refreshStatus when given an id", async () => {
      const snapshot = createMockDownloadSnapshot({ id: "task-1" });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.selectDownload("task-1");

      expect(store.selectedId).toBe("task-1");
    });

    it("clears selection when given null", async () => {
      store.selectedId = "task-1";
      await store.selectDownload(null);

      expect(store.selectedId).toBeNull();
      expect(store.selectedSnapshot).toBeNull();
    });
  });

  // ── runPause ────────────────────────────────────────────────────────────

  describe("runPause", () => {
    it("calls pauseDownload with selectedId", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(snapshot);
      store.selectedId = "task-1";

      await store.runPause();

      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── runResume ───────────────────────────────────────────────────────────

  describe("runResume", () => {
    it("calls resumeDownload with selectedId", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockResumeDownload.mockResolvedValue(snapshot);
      store.selectedId = "task-1";

      await store.runResume();

      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── runCancel ───────────────────────────────────────────────────────────

  describe("runCancel", () => {
    it("calls cancelDownload with selectedId", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockCancelDownload.mockResolvedValue(snapshot);
      store.selectedId = "task-1";

      await store.runCancel();

      expect(mockCancelDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── runDeleteTask (remove) ──────────────────────────────────────────────

  describe("runDeleteTask", () => {
    it("calls removeDownload with the given id", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockRemoveDownload.mockResolvedValue(snapshot);

      await store.runDeleteTask("task-1");

      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── runDeleteTaskPermanently (purge) ────────────────────────────────────

  describe("runDeleteTaskPermanently", () => {
    it("calls purgeDownload with the given id", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockPurgeDownload.mockResolvedValue(snapshot);

      await store.runDeleteTaskPermanently("task-1");

      expect(mockPurgeDownload).toHaveBeenCalledWith("task-1");
    });
  });

  // ── runOpenInExplorer ───────────────────────────────────────────────────

  describe("runOpenInExplorer", () => {
    it("calls openDownloadInExplorer with the given id", async () => {
      mockOpenDownloadInExplorer.mockResolvedValue(undefined);

      await store.runOpenInExplorer("task-1");

      expect(mockOpenDownloadInExplorer).toHaveBeenCalledWith("task-1");
    });
  });

  // ── Error handling ──────────────────────────────────────────────────────

  describe("error handling", () => {
    it("handles pauseDownload error", async () => {
      mockPauseDownload.mockRejectedValue(new Error("Backend unavailable"));
      store.selectedId = "task-1";

      await store.runPause();

      expect(store.actionName).toBe("");
    });

    it("resets actionName after successful action", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(snapshot);
      store.selectedId = "task-1";

      await store.runPause();

      expect(store.actionName).toBe("");
    });
  });

  // ── actionName lifecycle ────────────────────────────────────────────────

  describe("actionName lifecycle", () => {
    it("sets actionName during pause and resets after", async () => {
      let resolvePromise!: (snapshot: DownloadSnapshot) => void;
      const pendingPromise = new Promise<DownloadSnapshot>((resolve) => {
        resolvePromise = resolve;
      });
      mockPauseDownload.mockReturnValue(pendingPromise);
      store.selectedId = "task-1";

      const promise = store.runPause();

      expect(store.actionName).toBe("Pause");

      resolvePromise(createMockDownloadSnapshot({ id: "task-1" }));
      await promise;

      expect(store.actionName).toBe("");
    });
  });

  // ── runPauseAll ─────────────────────────────────────────────────────

  describe("runPauseAll", () => {
    it("pauses only downloading tasks", async () => {
      const downloading1 = DownloadPresets.downloading({ id: "task-1", fileName: "alpha.zip" });
      const paused1 = DownloadPresets.paused({ id: "task-2", fileName: "beta.zip" });
      const completed1 = DownloadPresets.completed({ id: "task-3", fileName: "gamma.zip" });
      const downloading2 = DownloadPresets.downloading({ id: "task-4", fileName: "delta.zip" });

      const snap1 = createMockDownloadSnapshot({ id: "task-1", state: "paused" });
      const snap2 = createMockDownloadSnapshot({ id: "task-4", state: "paused" });
      mockPauseDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      // Set up store downloads directly
      store.downloads.push(downloading1, paused1, completed1, downloading2);

      await store.runPauseAll();

      expect(mockPauseDownload).toHaveBeenCalledTimes(2);
      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
      expect(mockPauseDownload).toHaveBeenCalledWith("task-4");
      expect(store.actionName).toBe("");
    });

    it("does nothing when no tasks are downloading", async () => {
      store.downloads.push(
        DownloadPresets.paused({ id: "task-1" }),
        DownloadPresets.completed({ id: "task-2" }),
        DownloadPresets.failed({ id: "task-3" }),
      );

      await store.runPauseAll();

      expect(mockPauseDownload).not.toHaveBeenCalled();
      expect(store.actionName).toBe("");
    });
  });

  // ── runResumeAll ────────────────────────────────────────────────────

  describe("runResumeAll", () => {
    it("resumes only paused tasks", async () => {
      const paused1 = DownloadPresets.paused({ id: "task-1", fileName: "alpha.zip" });
      const downloading1 = DownloadPresets.downloading({ id: "task-2", fileName: "beta.zip" });
      const paused2 = DownloadPresets.paused({ id: "task-3", fileName: "gamma.zip" });

      const snap1 = createMockDownloadSnapshot({ id: "task-1", state: "downloading" });
      const snap2 = createMockDownloadSnapshot({ id: "task-3", state: "downloading" });
      mockResumeDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      store.downloads.push(paused1, downloading1, paused2);

      await store.runResumeAll();

      expect(mockResumeDownload).toHaveBeenCalledTimes(2);
      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
      expect(mockResumeDownload).toHaveBeenCalledWith("task-3");
      expect(store.actionName).toBe("");
    });
  });

  // ── runClearCompleted ───────────────────────────────────────────────

  describe("runClearCompleted", () => {
    it("clears only completed tasks", async () => {
      const completed1 = DownloadPresets.completed({ id: "task-1", fileName: "alpha.zip" });
      const downloading1 = DownloadPresets.downloading({ id: "task-2", fileName: "beta.zip" });
      const completed2 = DownloadPresets.completed({ id: "task-3", fileName: "gamma.zip" });

      const snap1 = createMockDownloadSnapshot({ id: "task-1" });
      const snap2 = createMockDownloadSnapshot({ id: "task-3" });
      mockRemoveDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      store.downloads.push(completed1, downloading1, completed2);

      await store.runClearCompleted();

      expect(mockRemoveDownload).toHaveBeenCalledTimes(2);
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-3");
      expect(store.actionName).toBe("");
    });
  });

  // ── runBatchDelete ──────────────────────────────────────────────────

  describe("runBatchDelete", () => {
    it("removes all specified tasks", async () => {
      const snap1 = createMockDownloadSnapshot({ id: "task-1" });
      const snap2 = createMockDownloadSnapshot({ id: "task-2" });
      mockRemoveDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      store.downloads.push(
        DownloadPresets.downloading({ id: "task-1", fileName: "alpha.zip" }),
        DownloadPresets.paused({ id: "task-2", fileName: "beta.zip" }),
        DownloadPresets.completed({ id: "task-3", fileName: "gamma.zip" }),
      );

      await store.runBatchDelete(["task-1", "task-2"]);

      expect(mockRemoveDownload).toHaveBeenCalledTimes(2);
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-2");
      expect(store.actionName).toBe("");
    });

    it("returns early when given an empty array", async () => {
      await store.runBatchDelete([]);
      expect(mockRemoveDownload).not.toHaveBeenCalled();
    });
  });

  // ── runCopyLink ─────────────────────────────────────────────────────

  describe("runCopyLink", () => {
    let mockClipboardWriteText: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      mockClipboardWriteText = vi.fn().mockResolvedValue(undefined);
      vi.stubGlobal(
        "navigator",
        Object.assign({}, navigator, {
          clipboard: { writeText: mockClipboardWriteText },
        }),
      );
    });

    afterEach(() => {
      vi.unstubAllGlobals();
    });

    it("copies link from selectedSnapshot when ids match", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        url: "https://example.com/file.zip",
      });
      store.selectedSnapshot = snapshot;
      store.selectedId = "task-1";

      await store.runCopyLink("task-1");

      expect(mockClipboardWriteText).toHaveBeenCalledWith("https://example.com/file.zip");
    });

    it("copies link from downloads list when selectedSnapshot ids do not match", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-999",
        url: "https://example.com/other.zip",
      });
      const task = DownloadPresets.downloading({
        id: "task-1",
        url: "https://example.com/myfile.zip",
      });

      store.selectedSnapshot = snapshot;
      store.downloads.push(task);

      await store.runCopyLink("task-1");

      expect(mockClipboardWriteText).toHaveBeenCalledWith("https://example.com/myfile.zip");
    });

    it("sets error when target has no url", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        url: "",
      });
      store.selectedSnapshot = snapshot;

      await store.runCopyLink("task-1");

      expect(mockClipboardWriteText).not.toHaveBeenCalled();
    });
  });
});
