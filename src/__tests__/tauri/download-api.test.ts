import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

import { invoke } from "#invoke";
import { createMockInvoke, resetTauriMocks, mockTauriCommandValue } from "../mocks/tauri-mock";
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
  previewTorrent,
  getBtFiles,
  updateBtFiles,
} from "../../lib/tauri/download-api";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  resetTauriMocks();
  mockInvoke.mockImplementation(createMockInvoke());
  vi.clearAllMocks();
});

describe("download-api", () => {
  it("startDownload calls download_start with request", async () => {
    const request = { url: "https://example.com/file.zip", destinationDir: "/tmp" };
    const expectedId = { kind: "http", id: "new-id" };
    mockTauriCommandValue("download_start", expectedId);

    const result = await startDownload(request);

    expect(mockInvoke).toHaveBeenCalledWith("download_start", { request });
    expect(result.id).toBe("new-id");
  });

  it("pauseDownload calls download_pause with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "paused" };
    mockTauriCommandValue("download_pause", snapshot);

    const result = await pauseDownload(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_pause", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("resumeDownload calls download_resume with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "downloading" };
    mockTauriCommandValue("download_resume", snapshot);

    const result = await resumeDownload(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_resume", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("cancelDownload calls download_cancel with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "cancelled" };
    mockTauriCommandValue("download_cancel", snapshot);

    const result = await cancelDownload(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_cancel", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("removeDownload calls download_remove with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "removed" };
    mockTauriCommandValue("download_remove", snapshot);

    const result = await removeDownload(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_remove", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("purgeDownload calls download_purge with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "purged" };
    mockTauriCommandValue("download_purge", snapshot);

    const result = await purgeDownload(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_purge", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("openDownloadInExplorer calls download_open_in_explorer with downloadId", async () => {
    const downloadId = "abc123";
    mockTauriCommandValue("download_open_in_explorer", undefined);

    const result = await openDownloadInExplorer(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_open_in_explorer", { downloadId });
    expect(result).toBeUndefined();
  });

  it("getDownloadStatus calls download_status with downloadId", async () => {
    const downloadId = "abc123";
    const snapshot = { id: downloadId, status: "downloading" };
    mockTauriCommandValue("download_status", snapshot);

    const result = await getDownloadStatus(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("download_status", { downloadId });
    expect(result).toEqual(snapshot);
  });

  it("listDownloads calls download_list", async () => {
    const summaries = [
      { id: "1", url: "https://example.com/1.zip", status: "completed" },
      { id: "2", url: "https://example.com/2.zip", status: "downloading" },
    ];
    mockTauriCommandValue("download_list", summaries);

    const result = await listDownloads();

    expect(mockInvoke).toHaveBeenCalledWith("download_list");
    expect(result).toEqual(summaries);
  });

  it("getBtRuntimeStatus calls bt_runtime_status", async () => {
    const status = { running: true, port: 6881 };
    mockTauriCommandValue("bt_runtime_status", status);

    const result = await getBtRuntimeStatus();

    expect(mockInvoke).toHaveBeenCalledWith("bt_runtime_status");
    expect(result).toEqual(status);
  });

  it("getBtPeers calls bt_get_peers with downloadId", async () => {
    const downloadId = "abc";
    const peers = [{ ip: "1.2.3.4", port: 6881 }];
    mockTauriCommandValue("bt_get_peers", peers);

    const result = await getBtPeers(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("bt_get_peers", { downloadId });
    expect(result).toEqual(peers);
  });

  it("getBtTrackers calls bt_get_trackers with downloadId", async () => {
    const downloadId = "abc";
    const trackers = [{ url: "udp://tracker.example.com:6969", status: "working" }];
    mockTauriCommandValue("bt_get_trackers", trackers);

    const result = await getBtTrackers(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("bt_get_trackers", { downloadId });
    expect(result).toEqual(trackers);
  });

  it("getBtPieces calls bt_get_pieces with downloadId", async () => {
    const downloadId = "abc";
    const pieces = [{ index: 0, status: "downloaded" }];
    mockTauriCommandValue("bt_get_pieces", pieces);

    const result = await getBtPieces(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("bt_get_pieces", { downloadId });
    expect(result).toEqual(pieces);
  });

  it("setBtSpeedLimit calls bt_set_speed_limit with downloadId and limits", async () => {
    const downloadId = "abc";
    const downloadLimitBps = 102400;
    const uploadLimitBps = 51200;
    mockTauriCommandValue("bt_set_speed_limit", undefined);

    const result = await setBtSpeedLimit(downloadId, downloadLimitBps, uploadLimitBps);

    expect(mockInvoke).toHaveBeenCalledWith("bt_set_speed_limit", {
      downloadId,
      downloadLimitBps,
      uploadLimitBps,
    });
    expect(result).toBeUndefined();
  });

  it("setBtSpeedLimit calls bt_set_speed_limit with partial limits", async () => {
    const downloadId = "abc";
    mockTauriCommandValue("bt_set_speed_limit", undefined);

    const result = await setBtSpeedLimit(downloadId, 204800);

    expect(mockInvoke).toHaveBeenCalledWith("bt_set_speed_limit", {
      downloadId,
      downloadLimitBps: 204800,
      uploadLimitBps: undefined,
    });
    expect(result).toBeUndefined();
  });

  it("previewTorrent calls bt_preview_torrent with source", async () => {
    const source = "/path/to/file.torrent";
    const files = [{ path: "file1.txt", length: 100 }];
    mockTauriCommandValue("bt_preview_torrent", files);

    const result = await previewTorrent(source);

    expect(mockInvoke).toHaveBeenCalledWith("bt_preview_torrent", { source });
    expect(result).toEqual(files);
  });

  it("getBtFiles calls get_bt_files with downloadId", async () => {
    const downloadId = "abc";
    const files = [{ path: "movie.mp4", included: true }];
    mockTauriCommandValue("get_bt_files", files);

    const result = await getBtFiles(downloadId);

    expect(mockInvoke).toHaveBeenCalledWith("get_bt_files", { downloadId });
    expect(result).toEqual(files);
  });

  it("updateBtFiles calls update_bt_files with downloadId and includedIndices", async () => {
    const downloadId = "abc";
    const includedIndices = [0, 2, 4];
    mockTauriCommandValue("update_bt_files", undefined);

    const result = await updateBtFiles(downloadId, includedIndices);

    expect(mockInvoke).toHaveBeenCalledWith("update_bt_files", {
      downloadId,
      includedIndices,
    });
    expect(result).toBeUndefined();
  });
});
