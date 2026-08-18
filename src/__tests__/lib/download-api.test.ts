import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

import { invoke } from "#invoke";
import {
  startDownload,
  pauseDownload,
  resumeDownload,
  cancelDownload,
  removeDownload,
  purgeDownload,
  openDownloadInExplorer,
  getDownloadStatus,
  listDownloads,
  getBtRuntimeStatus,
  getBtPeers,
  getBtTrackers,
  getBtPieces,
  setBtSpeedLimit,
  setPriority,
  previewTorrent,
  getBtFiles,
  updateBtFiles,
} from "../../lib/tauri/download-api";

const mockInvoke = vi.mocked(invoke);

// ── Tests ───────────────────────────────────────────────────────────────────────

describe("download-api", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── startDownload ────────────────────────────────────────────────────

  describe("startDownload", () => {
    it("calls invoke with 'download_start' and the request object", async () => {
      const request = {
        url: "https://example.com/file.zip",
        destinationDir: "/downloads",
        fileName: "file.zip",
        kind: "http",
        userAgent: null,
        startPaused: false,
      } as const;
      mockInvoke.mockResolvedValue({ kind: "http", id: "task-abc" });

      const result = await startDownload(request);

      expect(mockInvoke).toHaveBeenCalledWith("download_start", { request });
      expect(result).toEqual({ kind: "http", id: "task-abc" });
    });
  });

  // ── pauseDownload ─────────────────────────────────────────────────────

  describe("pauseDownload", () => {
    it("calls invoke with 'download_pause' and { downloadId }", async () => {
      const snapshot = { id: "task-1", state: "paused" };
      mockInvoke.mockResolvedValue(snapshot);

      const result = await pauseDownload("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_pause", { downloadId: "task-1" });
      expect(result).toEqual(snapshot);
    });
  });

  // ── resumeDownload ────────────────────────────────────────────────────

  describe("resumeDownload", () => {
    it("calls invoke with 'download_resume' and { downloadId }", async () => {
      const snapshot = { id: "task-1", state: "downloading" };
      mockInvoke.mockResolvedValue(snapshot);

      const result = await resumeDownload("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_resume", { downloadId: "task-1" });
      expect(result).toEqual(snapshot);
    });
  });

  // ── cancelDownload ────────────────────────────────────────────────────

  describe("cancelDownload", () => {
    it("calls invoke with 'download_cancel' and { downloadId }", async () => {
      const snapshot = { id: "task-1", state: "cancelled" };
      mockInvoke.mockResolvedValue(snapshot);

      const result = await cancelDownload("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_cancel", { downloadId: "task-1" });
      expect(result).toEqual(snapshot);
    });
  });

  // ── removeDownload ────────────────────────────────────────────────────

  describe("removeDownload", () => {
    it("calls invoke with 'download_remove' and { downloadId }", async () => {
      mockInvoke.mockResolvedValue({ id: "task-1" });

      await removeDownload("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_remove", { downloadId: "task-1" });
    });
  });

  // ── purgeDownload ─────────────────────────────────────────────────────

  describe("purgeDownload", () => {
    it("calls invoke with 'download_purge' and { downloadId }", async () => {
      mockInvoke.mockResolvedValue({ id: "task-1" });

      await purgeDownload("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_purge", { downloadId: "task-1" });
    });
  });

  // ── openDownloadInExplorer ────────────────────────────────────────────

  describe("openDownloadInExplorer", () => {
    it("calls invoke with 'download_open_in_explorer' and { downloadId }", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await openDownloadInExplorer("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_open_in_explorer", {
        downloadId: "task-1",
      });
    });
  });

  // ── getDownloadStatus ─────────────────────────────────────────────────

  describe("getDownloadStatus", () => {
    it("calls invoke with 'download_status' and { downloadId }", async () => {
      const status = { id: "task-1", state: "downloading", progress: 0.5 };
      mockInvoke.mockResolvedValue(status);

      const result = await getDownloadStatus("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("download_status", { downloadId: "task-1" });
      expect(result).toEqual(status);
    });

    it("propagates errors from the backend", async () => {
      mockInvoke.mockRejectedValue(new Error("Task not found"));

      await expect(getDownloadStatus("nonexistent")).rejects.toThrow("Task not found");
    });
  });

  // ── listDownloads ─────────────────────────────────────────────────────

  describe("listDownloads", () => {
    it("calls invoke with 'download_list' and no args", async () => {
      const summaries = [{ id: "task-1", fileName: "file.zip" }];
      mockInvoke.mockResolvedValue(summaries);

      const result = await listDownloads();

      expect(mockInvoke).toHaveBeenCalledWith("download_list");
      expect(result).toEqual(summaries);
    });

    it("returns empty array when there are no downloads", async () => {
      mockInvoke.mockResolvedValue([]);

      const result = await listDownloads();

      expect(result).toEqual([]);
    });
  });

  // ── getBtRuntimeStatus ────────────────────────────────────────────────

  describe("getBtRuntimeStatus", () => {
    it("calls invoke with 'bt_runtime_status' and no args", async () => {
      const status = { connected: true, torrentCount: 3, peerCount: 10 };
      mockInvoke.mockResolvedValue(status);

      const result = await getBtRuntimeStatus();

      expect(mockInvoke).toHaveBeenCalledWith("bt_runtime_status");
      expect(result).toEqual(status);
    });
  });

  // ── getBtPeers ────────────────────────────────────────────────────────

  describe("getBtPeers", () => {
    it("calls invoke with 'bt_get_peers' and { downloadId }", async () => {
      const peers = [{ address: "1.2.3.4:6881", client: "qBittorrent" }];
      mockInvoke.mockResolvedValue(peers);

      const result = await getBtPeers("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("bt_get_peers", { downloadId: "task-1" });
      expect(result).toEqual(peers);
    });
  });

  // ── getBtTrackers ─────────────────────────────────────────────────────

  describe("getBtTrackers", () => {
    it("calls invoke with 'bt_get_trackers' and { downloadId }", async () => {
      const trackers = [{ url: "udp://tracker.example:6969" }];
      mockInvoke.mockResolvedValue(trackers);

      const result = await getBtTrackers("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("bt_get_trackers", { downloadId: "task-1" });
      expect(result).toEqual(trackers);
    });
  });

  // ── getBtPieces ───────────────────────────────────────────────────────

  describe("getBtPieces", () => {
    it("calls invoke with 'bt_get_pieces' and { downloadId }", async () => {
      const pieces = [{ index: 0, completed: true }];
      mockInvoke.mockResolvedValue(pieces);

      const result = await getBtPieces("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("bt_get_pieces", { downloadId: "task-1" });
      expect(result).toEqual(pieces);
    });
  });

  // ── setBtSpeedLimit ───────────────────────────────────────────────────

  describe("setBtSpeedLimit", () => {
    it("calls invoke with 'bt_set_speed_limit' and all three args", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await setBtSpeedLimit("task-1", 1024, 512);

      expect(mockInvoke).toHaveBeenCalledWith("bt_set_speed_limit", {
        downloadId: "task-1",
        downloadLimitBps: 1024,
        uploadLimitBps: 512,
      });
    });

    it("passes undefined for omitted optional limits", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await setBtSpeedLimit("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("bt_set_speed_limit", {
        downloadId: "task-1",
        downloadLimitBps: undefined,
        uploadLimitBps: undefined,
      });
    });

    it("passes only download limit when upload is omitted", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await setBtSpeedLimit("task-1", 2048);

      expect(mockInvoke).toHaveBeenCalledWith("bt_set_speed_limit", {
        downloadId: "task-1",
        downloadLimitBps: 2048,
        uploadLimitBps: undefined,
      });
    });
  });

  // ── setPriority ───────────────────────────────────────────────────────

  describe("setPriority", () => {
    it.each<[string, "low" | "normal" | "high"]>([
      ["calls invoke with 'download_set_priority' and { downloadId, priority }", "high"],
      ["accepts 'low' priority", "low"],
      ["accepts 'normal' priority", "normal"],
    ])("%s", async (_title, priority) => {
      mockInvoke.mockResolvedValue(undefined);

      await setPriority("task-1", priority);

      expect(mockInvoke).toHaveBeenCalledWith("download_set_priority", {
        downloadId: "task-1",
        priority,
      });
    });
  });

  // ── previewTorrent ────────────────────────────────────────────────────

  describe("previewTorrent", () => {
    it("calls invoke with 'bt_preview_torrent' and { source }", async () => {
      const entries = [{ index: 0, path: "file.txt", size: 100 }];
      mockInvoke.mockResolvedValue(entries);

      const result = await previewTorrent("/path/to/torrent.torrent");

      expect(mockInvoke).toHaveBeenCalledWith("bt_preview_torrent", {
        source: "/path/to/torrent.torrent",
      });
      expect(result).toEqual(entries);
    });
  });

  // ── getBtFiles ────────────────────────────────────────────────────────

  describe("getBtFiles", () => {
    it("calls invoke with 'get_bt_files' and { downloadId }", async () => {
      const files = [
        { index: 0, path: "file.txt", size: 100, downloadedBytes: 50, included: true },
      ];
      mockInvoke.mockResolvedValue(files);

      const result = await getBtFiles("task-1");

      expect(mockInvoke).toHaveBeenCalledWith("get_bt_files", { downloadId: "task-1" });
      expect(result).toEqual(files);
    });
  });

  // ── updateBtFiles ─────────────────────────────────────────────────────

  describe("updateBtFiles", () => {
    it("calls invoke with 'update_bt_files' and { downloadId, includedIndices }", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await updateBtFiles("task-1", [0, 2, 4]);

      expect(mockInvoke).toHaveBeenCalledWith("update_bt_files", {
        downloadId: "task-1",
        includedIndices: [0, 2, 4],
      });
    });

    it("accepts an empty includedIndices array", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await updateBtFiles("task-1", []);

      expect(mockInvoke).toHaveBeenCalledWith("update_bt_files", {
        downloadId: "task-1",
        includedIndices: [],
      });
    });
  });
});
