import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { ref } from "vue";

vi.mock("../../lib/tauri/download-api", () => ({
  getBtPeers: vi.fn(),
  getBtTrackers: vi.fn(),
  getBtPieces: vi.fn(),
  getBtFiles: vi.fn(),
  updateBtFiles: vi.fn(),
}));

import {
  getBtPeers,
  getBtTrackers,
  getBtPieces,
  getBtFiles,
  updateBtFiles,
} from "../../lib/tauri/download-api";
import { useBtInspector } from "../../composables/useBtInspector";
import type { BtFileStatus, BtPeerInfo, BtPieceInfo, BtTrackerInfo } from "../../types/download";

// ── Mock data ──────────────────────────────────────────────────────────────────

const mockPeer: BtPeerInfo = {
  address: "1.2.3.4:6881",
  client: "qBittorrent",
  flags: "",
  downloadSpeed: 1024,
  uploadSpeed: 512,
  progress: 0.5,
};

const mockFiles: BtFileStatus[] = [
  { index: 0, path: "file1.txt", size: 100, downloadedBytes: 50, included: true },
  { index: 1, path: "file2.txt", size: 200, downloadedBytes: 0, included: true },
  { index: 2, path: "file3.txt", size: 300, downloadedBytes: 0, included: false },
];

const mockTrackers: BtTrackerInfo[] = [{ url: "udp://tracker.example.com:6969" }];

const mockPieces: BtPieceInfo[] = [
  { index: 0, completed: true },
  { index: 1, completed: false },
];

// ── Mock function references ────────────────────────────────────────────────────

const mockGetBtPeers = vi.mocked(getBtPeers);
const mockGetBtTrackers = vi.mocked(getBtTrackers);
const mockGetBtPieces = vi.mocked(getBtPieces);
const mockGetBtFiles = vi.mocked(getBtFiles);
const mockUpdateBtFiles = vi.mocked(updateBtFiles);

// ── Helpers ─────────────────────────────────────────────────────────────────────

function flushPromises() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

// ── Tests ───────────────────────────────────────────────────────────────────────

describe("useBtInspector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ── Initial state ─────────────────────────────────────────────────────────

  describe("initial state", () => {
    it("returns empty arrays, loading false, errors empty when taskId is null", () => {
      const taskId = ref<string | null>(null);
      const inspector = useBtInspector(taskId);

      expect(inspector.files.value).toEqual([]);
      expect(inspector.peers.value).toEqual([]);
      expect(inspector.trackers.value).toEqual([]);
      expect(inspector.pieces.value).toEqual([]);

      expect(inspector.isLoading).toEqual({
        files: false,
        peers: false,
        trackers: false,
        pieces: false,
      });

      expect(inspector.errors).toEqual({
        files: "",
        peers: "",
        trackers: "",
        pieces: "",
      });

      expect(inspector.isUpdatingFiles.value).toBe(false);
    });

    it("does not call any fetcher when taskId is null", () => {
      const taskId = ref<string | null>(null);
      useBtInspector(taskId);

      expect(mockGetBtPeers).not.toHaveBeenCalled();
      expect(mockGetBtTrackers).not.toHaveBeenCalled();
      expect(mockGetBtPieces).not.toHaveBeenCalled();
      expect(mockGetBtFiles).not.toHaveBeenCalled();
    });
  });

  // ── Auto-fetch on taskId change ─────────────────────────────────────────

  describe("auto-fetch on taskId change", () => {
    it("fetches peers, trackers, pieces automatically when taskId is truthy (immediate watch)", async () => {
      mockGetBtPeers.mockResolvedValue([mockPeer]);
      mockGetBtTrackers.mockResolvedValue(mockTrackers);
      mockGetBtPieces.mockResolvedValue(mockPieces);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      await flushPromises();

      expect(mockGetBtPeers).toHaveBeenCalledWith("task-1");
      expect(mockGetBtTrackers).toHaveBeenCalledWith("task-1");
      expect(mockGetBtPieces).toHaveBeenCalledWith("task-1");

      expect(inspector.peers.value).toEqual([mockPeer]);
      expect(inspector.trackers.value).toEqual(mockTrackers);
      expect(inspector.pieces.value).toEqual(mockPieces);
    });

    it("does NOT fetch files automatically on taskId change", async () => {
      mockGetBtPeers.mockResolvedValue([]);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      useBtInspector(taskId);

      await flushPromises();

      expect(mockGetBtFiles).not.toHaveBeenCalled();
    });
  });

  // ── Manual fetch ─────────────────────────────────────────────────────────

  describe("manual fetch", () => {
    it("fetchFiles calls getBtFiles and updates files ref", async () => {
      mockGetBtFiles.mockResolvedValue(mockFiles);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      // Let auto-fetch complete
      await flushPromises();

      await inspector.fetchFiles();

      expect(mockGetBtFiles).toHaveBeenCalledWith("task-1");
      expect(inspector.files.value).toEqual(mockFiles);
    });

    it("fetchPeers calls getBtPeers and updates peers ref", async () => {
      mockGetBtPeers.mockResolvedValue([mockPeer]);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      // Let auto-fetch complete (stats are cleared below)
      await flushPromises();
      mockGetBtPeers.mockClear();

      await inspector.fetchPeers();

      expect(mockGetBtPeers).toHaveBeenCalledWith("task-1");
      expect(inspector.peers.value).toEqual([mockPeer]);
    });

    it("fetchTrackers calls getBtTrackers and updates trackers ref", async () => {
      mockGetBtTrackers.mockResolvedValue(mockTrackers);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      mockGetBtTrackers.mockClear();

      await inspector.fetchTrackers();

      expect(mockGetBtTrackers).toHaveBeenCalledWith("task-1");
      expect(inspector.trackers.value).toEqual(mockTrackers);
    });

    it("fetchPieces calls getBtPieces and updates pieces ref", async () => {
      mockGetBtPieces.mockResolvedValue(mockPieces);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      mockGetBtPieces.mockClear();

      await inspector.fetchPieces();

      expect(mockGetBtPieces).toHaveBeenCalledWith("task-1");
      expect(inspector.pieces.value).toEqual(mockPieces);
    });
  });

  // ── Loading state ────────────────────────────────────────────────────────

  describe("loading state", () => {
    it("sets isLoading to true during fetch and false after", async () => {
      let resolvePeers!: (data: BtPeerInfo[]) => void;
      mockGetBtPeers.mockImplementationOnce(
        () => new Promise<BtPeerInfo[]>((r) => { resolvePeers = r; }),
      );
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      // isLoading.peers should be true while fetch is pending
      expect(inspector.isLoading.peers).toBe(true);

      // Resolve the deferred
      resolvePeers([]);
      await flushPromises();

      expect(inspector.isLoading.peers).toBe(false);
    });

    it("tracks per-tab loading state independently", async () => {
      let resolvePeers!: (data: BtPeerInfo[]) => void;
      mockGetBtPeers.mockImplementationOnce(
        () => new Promise<BtPeerInfo[]>((r) => { resolvePeers = r; }),
      );
      // Let trackers/pieces resolve immediately
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      // peers is loading, but files was never started
      expect(inspector.isLoading.peers).toBe(true);
      expect(inspector.isLoading.files).toBe(false);

      // trackers and pieces resolved immediately above (microtask)
      // but we need to flush for their finally blocks
      await flushPromises();
      expect(inspector.isLoading.trackers).toBe(false);
      expect(inspector.isLoading.pieces).toBe(false);

      // peers still pending
      expect(inspector.isLoading.peers).toBe(true);

      resolvePeers([]);
      await flushPromises();

      expect(inspector.isLoading.peers).toBe(false);
    });
  });

  // ── Error handling ──────────────────────────────────────────────────────

  describe("error handling", () => {
    it("sets errors state and logs to console when fetch fails", async () => {
      const testError = new Error("Network error");
      mockGetBtPeers.mockResolvedValue([]);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockRejectedValue(testError);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      await flushPromises();

      expect(inspector.errors.pieces).toBe("Error: Network error");
      expect(console.error).toHaveBeenCalledWith(
        "[useBtInspector] pieces fetch failed:",
        testError,
      );
    });

    it("sets errors for string rejections", async () => {
      mockGetBtPeers.mockResolvedValue([]);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockRejectedValue("String error");

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      await flushPromises();

      expect(inspector.errors.pieces).toBe("String error");
    });

    it("only sets error for the tab that failed", async () => {
      mockGetBtPeers.mockRejectedValue(new Error("Peers fail"));
      mockGetBtTrackers.mockResolvedValue(mockTrackers);
      mockGetBtPieces.mockResolvedValue(mockPieces);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);

      await flushPromises();

      // peers failed
      expect(inspector.errors.peers).toBe("Error: Peers fail");
      // others succeeded — errors remain empty
      expect(inspector.errors.trackers).toBe("");
      expect(inspector.errors.pieces).toBe("");
      expect(inspector.errors.files).toBe("");
    });
  });

  // ── Stale callback rejection ────────────────────────────────────────────

  describe("stale callback rejection", () => {
    it("fast concurrent calls: only the latest callback updates state", async () => {
      // Set up auto-fetch to resolve immediately
      mockGetBtPeers.mockResolvedValue([]);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      const { peers, fetchPeers, isLoading } = useBtInspector(taskId);

      await flushPromises(); // auto-fetch done

      // Now clear stats and set up deferred mocks
      mockGetBtPeers.mockClear();

      const freshData: BtPeerInfo[] = [
        { ...mockPeer, address: "9.9.9.9:6881" },
      ];
      const staleData: BtPeerInfo[] = [
        { ...mockPeer, address: "1.1.1.1:6881" },
      ];

      let resolveStale!: (v: BtPeerInfo[]) => void;
      let resolveFresh!: (v: BtPeerInfo[]) => void;

      mockGetBtPeers
        .mockImplementationOnce(
          () => new Promise<BtPeerInfo[]>((r) => { resolveStale = r; }),
        )
        .mockImplementationOnce(
          () => new Promise<BtPeerInfo[]>((r) => { resolveFresh = r; }),
        );

      // Start two overlapping fetches
      const pStale = fetchPeers();
      const pFresh = fetchPeers();

      // Resolve the stale one first
      resolveStale(staleData);
      await pStale;

      // peers should NOT have been updated with stale data
      expect(peers.value).toEqual([]);
      // But isLoading is still true because the second call hasn't resolved yet
      expect(isLoading.peers).toBe(true);

      // Resolve the fresh one
      resolveFresh(freshData);
      await pFresh;

      expect(peers.value).toEqual(freshData);
      expect(isLoading.peers).toBe(false);
    });

    it("sequential call: stale result is rejected when a newer fetch completes first", async () => {
      mockGetBtPeers.mockResolvedValue([]);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      const { peers, fetchPeers } = useBtInspector(taskId);
      await flushPromises();
      mockGetBtPeers.mockClear();

      const dataA: BtPeerInfo[] = [{ ...mockPeer, address: "A:6881" }];
      const dataB: BtPeerInfo[] = [{ ...mockPeer, address: "B:6881" }];

      let resolveA!: (v: BtPeerInfo[]) => void;
      let resolveB!: (v: BtPeerInfo[]) => void;

      mockGetBtPeers
        .mockImplementationOnce(
          () => new Promise<BtPeerInfo[]>((r) => { resolveA = r; }),
        )
        .mockImplementationOnce(
          () => new Promise<BtPeerInfo[]>((r) => { resolveB = r; }),
        );

      const pA = fetchPeers(); // version += 1
      const pB = fetchPeers(); // version += 1 again — makes pA stale

      // Resolve B first (the "fresh" call completes before A)
      resolveB(dataB);
      await pB;
      expect(peers.value).toEqual(dataB);

      // Now resolve A (stale — version no longer matches)
      resolveA(dataA);
      await pA;
      // peers should still be dataB, not overwritten by stale dataA
      expect(peers.value).toEqual(dataB);
    });
  });

  // ── clear() ─────────────────────────────────────────────────────────────

  describe("clear()", () => {
    it("resets all arrays to []", async () => {
      mockGetBtPeers.mockResolvedValue([mockPeer]);
      mockGetBtTrackers.mockResolvedValue(mockTrackers);
      mockGetBtPieces.mockResolvedValue(mockPieces);
      mockGetBtFiles.mockResolvedValue(mockFiles);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Confirm data is loaded
      expect(inspector.peers.value).toEqual([mockPeer]);
      expect(inspector.files.value).toEqual(mockFiles);

      inspector.clear();

      expect(inspector.files.value).toEqual([]);
      expect(inspector.peers.value).toEqual([]);
      expect(inspector.trackers.value).toEqual([]);
      expect(inspector.pieces.value).toEqual([]);
    });
  });

  // ── Watch cleanup ───────────────────────────────────────────────────────

  describe("watch cleanup", () => {
    it("calls clear() when taskId becomes null", async () => {
      mockGetBtPeers.mockResolvedValue([mockPeer]);
      mockGetBtTrackers.mockResolvedValue(mockTrackers);
      mockGetBtPieces.mockResolvedValue(mockPieces);

      const taskId = ref<string | null>("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();

      expect(inspector.peers.value).toEqual([mockPeer]);

      // Change taskId to null
      taskId.value = null;
      await flushPromises();

      expect(inspector.files.value).toEqual([]);
      expect(inspector.peers.value).toEqual([]);
      expect(inspector.trackers.value).toEqual([]);
      expect(inspector.pieces.value).toEqual([]);
    });

    it("does not call clear() when taskId changes to a different truthy value", async () => {
      const peerData1 = [{ ...mockPeer, address: "1.1.1.1:6881" }];
      const peerData2 = [{ ...mockPeer, address: "2.2.2.2:6881" }];

      mockGetBtPeers.mockResolvedValue(peerData1);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();

      expect(inspector.peers.value).toEqual(peerData1);

      // Change to a different truthy id — should NOT clear, should re-fetch
      mockGetBtPeers.mockClear();
      mockGetBtPeers.mockResolvedValue(peerData2);
      mockGetBtTrackers.mockResolvedValue([]);
      mockGetBtPieces.mockResolvedValue([]);

      taskId.value = "task-2";
      await flushPromises();

      // Data should be replaced, not cleared
      expect(inspector.peers.value).toEqual(peerData2);
    });
  });

  // ── toggleFileInclusion ─────────────────────────────────────────────────

  describe("toggleFileInclusion", () => {
    beforeEach(() => {
      // Populate files for toggle tests
      mockGetBtFiles.mockResolvedValue(mockFiles);
    });

    it("adds a file to included when currentlyIncluded is false", async () => {
      const filesWithExcluded = [
        { index: 0, path: "a.txt", size: 10, downloadedBytes: 0, included: true },
        { index: 1, path: "b.txt", size: 20, downloadedBytes: 0, included: false },
      ];
      mockGetBtFiles.mockResolvedValue(filesWithExcluded);
      mockUpdateBtFiles.mockResolvedValue(undefined);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Select the excluded file
      await inspector.toggleFileInclusion(1, false);

      // Called with both file indices
      expect(mockUpdateBtFiles).toHaveBeenCalledWith("task-1", [0, 1]);
      // Optimistic update: file 1 is now included
      expect(inspector.files.value[1].included).toBe(true);
    });

    it("removes a file from included when currentlyIncluded is true", async () => {
      const files = [
        { index: 0, path: "a.txt", size: 10, downloadedBytes: 0, included: true },
        { index: 1, path: "b.txt", size: 20, downloadedBytes: 0, included: true },
      ];
      mockGetBtFiles.mockResolvedValue(files);
      mockUpdateBtFiles.mockResolvedValue(undefined);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Deselect file 0
      await inspector.toggleFileInclusion(0, true);

      // Called with only file 1 remaining
      expect(mockUpdateBtFiles).toHaveBeenCalledWith("task-1", [1]);
      // Optimistic update: file 0 is now excluded
      expect(inspector.files.value[0].included).toBe(false);
    });

    it("prevents deselecting ALL files (guard)", async () => {
      const singleFile: BtFileStatus[] = [
        { index: 0, path: "only.txt", size: 100, downloadedBytes: 0, included: true },
      ];
      mockGetBtFiles.mockResolvedValue(singleFile);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      await inspector.toggleFileInclusion(0, true);

      // updateBtFiles should NOT be called (guard prevents)
      expect(mockUpdateBtFiles).not.toHaveBeenCalled();
      // File should still be included
      expect(inspector.files.value[0].included).toBe(true);
    });

    it("optimistically updates local state on success", async () => {
      const files = [
        { index: 0, path: "a.txt", size: 10, downloadedBytes: 0, included: true },
        { index: 1, path: "b.txt", size: 20, downloadedBytes: 0, included: true },
        { index: 2, path: "c.txt", size: 30, downloadedBytes: 0, included: false },
      ];
      mockGetBtFiles.mockResolvedValue(files);
      mockUpdateBtFiles.mockResolvedValue(undefined);

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Select file 2 (previously excluded)
      await inspector.toggleFileInclusion(2, false);

      // Optimistic: file 2 is now included
      expect(inspector.files.value[2].included).toBe(true);
      // isUpdatingFiles should be false after completion
      expect(inspector.isUpdatingFiles.value).toBe(false);
    });

    it("reverts on error by refetching files", async () => {
      const originalFiles: BtFileStatus[] = [
        { index: 0, path: "a.txt", size: 10, downloadedBytes: 0, included: true },
        { index: 1, path: "b.txt", size: 20, downloadedBytes: 0, included: true },
      ];
      mockGetBtFiles.mockResolvedValue(originalFiles);
      mockUpdateBtFiles.mockRejectedValue(new Error("Backend error"));

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Clear fetchFiles call count before toggle
      mockGetBtFiles.mockClear();
      // Restore the resolved value for the refetch call
      mockGetBtFiles.mockResolvedValue(originalFiles);

      await inspector.toggleFileInclusion(0, true);

      // updateBtFiles was called with the remaining file
      expect(mockUpdateBtFiles).toHaveBeenCalledWith("task-1", [1]);
      // fetchFiles was called to refresh (revert)
      expect(mockGetBtFiles).toHaveBeenCalledWith("task-1");
      // isUpdatingFiles reset
      expect(inspector.isUpdatingFiles.value).toBe(false);
      // The optimistic update did NOT happen (caught before it) —
      // refetch restored original state
      expect(inspector.files.value[0].included).toBe(true);
      expect(inspector.files.value).toEqual(originalFiles);
    });

    it("sets isUpdatingFiles to true during the call and false after", async () => {
      const files = [
        { index: 0, path: "a.txt", size: 10, downloadedBytes: 0, included: true },
        { index: 1, path: "b.txt", size: 20, downloadedBytes: 0, included: false },
      ];
      mockGetBtFiles.mockResolvedValue(files);

      let resolveUpdate!: () => void;
      mockUpdateBtFiles.mockImplementationOnce(
        () => new Promise<void>((r) => { resolveUpdate = r; }),
      );

      const taskId = ref("task-1");
      const inspector = useBtInspector(taskId);
      await flushPromises();
      await inspector.fetchFiles();

      // Start toggle — don't await yet so we can check intermediate state
      const togglePromise = inspector.toggleFileInclusion(1, false);

      // isUpdatingFiles should be true during the call
      expect(inspector.isUpdatingFiles.value).toBe(true);

      resolveUpdate();
      await togglePromise;

      expect(inspector.isUpdatingFiles.value).toBe(false);
    });

    it("is a no-op when taskId is null", async () => {
      mockGetBtFiles.mockResolvedValue(mockFiles);

      const taskId = ref<string | null>(null);
      const inspector = useBtInspector(taskId);
      await flushPromises();

      await inspector.toggleFileInclusion(0, true);

      expect(mockUpdateBtFiles).not.toHaveBeenCalled();
      expect(inspector.isUpdatingFiles.value).toBe(false);
    });
  });

  // ── Returned values ─────────────────────────────────────────────────────

  describe("returned values", () => {
    it("exposes all expected properties and methods", () => {
      const taskId = ref<string | null>(null);
      const inspector = useBtInspector(taskId);

      expect(inspector).toHaveProperty("files");
      expect(inspector).toHaveProperty("peers");
      expect(inspector).toHaveProperty("trackers");
      expect(inspector).toHaveProperty("pieces");
      expect(inspector).toHaveProperty("isLoading");
      expect(inspector).toHaveProperty("errors");
      expect(inspector).toHaveProperty("isUpdatingFiles");
      expect(inspector).toHaveProperty("fetchFiles");
      expect(inspector).toHaveProperty("fetchPeers");
      expect(inspector).toHaveProperty("fetchTrackers");
      expect(inspector).toHaveProperty("fetchPieces");
      expect(inspector).toHaveProperty("toggleFileInclusion");
      expect(inspector).toHaveProperty("clear");
      expect(typeof inspector.fetchFiles).toBe("function");
      expect(typeof inspector.toggleFileInclusion).toBe("function");
      expect(typeof inspector.clear).toBe("function");
    });
  });
});
