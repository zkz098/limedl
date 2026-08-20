import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

import { setupDownloadStoreMocks } from "../fixtures/download-store-mocks";
setupDownloadStoreMocks();

import { useDownloadStore } from "../../stores/download";
import {
  cancelDownload,
  getBtRuntimeStatus,
  getDownloadStatus,
  listDownloads,
  pauseDownload,
  resumeDownload,
  setPriority,
} from "../../lib/tauri/download-api";
import {
  createMockDownloadTask,
  createMockDownloadSnapshot,
  DownloadPresets,
  resetMockIds,
} from "../fixtures/downloads";
import type { AppSettings } from "../../types/settings";
import type { BtRuntimeStatus, DownloadSnapshot, DownloadSummary } from "../../types/download";

function createMinimalAppSettings(overrides?: Partial<AppSettings>): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    pet: { enabled: false, scale: 1, opacity: 1, keepAliveWhenMainHidden: true, model: "default" },
    appearance: {
      themeColor: "lime",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: false,
      showHeatmap: false,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: [],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "traditional",
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 4,
        maxThreadsPerTask: 2,
        minThreadsPerTask: 1,
        adaptiveProfile: "balanced",
      },
      chunkSizeStrategy: "adaptive",
      tailSprintEnabled: false,
      connectionWarmupEnabled: false,
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 3,
      defaultChecksum: "blake3",
      defaultUserAgent: "",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      antiLeechEnabled: false,
      antiLeechAction: "ban",
      antiLeechGraceSecs: 300,
      antiLeechRatio: 0.1,
      antiLeechBanSecs: 3600,
      antiLeechMaxUploadSlots: 4,
      seedChokingAlgorithm: "fastest_upload",
      chokingAlgorithm: "fixed_slots",
      maxUploadSlotsPerTorrent: 4,
      maxPeersPerTorrent: 128,
      smartBanMaxFailures: 3,
      smartBanParole: true,
      evictionBanDurationSecs: 600,
      dataContributionTimeoutSecs: 60,
      blocklistEnabled: false,
      blocklistPath: "",
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "",
      listenPort: null,
      listenPortRange: null,
      upnpEnabled: true,
      enableNatpmp: false,
      enableIpv6: false,
      enablePex: true,
      enableLsd: true,
      enableUtp: true,
      enableFastExtension: true,
      enableHolepunch: true,
      enableWebSeed: true,
      enableSuperSeeding: false,
      globalDownloadRateLimit: 0,
      globalUploadRateLimit: 0,
      preallocateMode: "none",
      encryptionMode: "enabled",
      maxDownloads: 5,
      maxSeeds: 2,
      maxTorrents: 10,
      activeLimit: 15,
    },
    logging: {
      enabled: false,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: false, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: false },
    ioBaseline: {
      bufferLimitMb: 256,
      gameModeBufferMb: 512,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 2,
      gameModeMaxParallel: 4,
      hddBufferEnabled: true,
      ssdWriteCombineMb: 0,
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    doubleClick: { onCompleted: "none", onUncompleted: "none" },
    speedLimitSchedule: [],
    ...overrides,
  };
}

const mockPauseDownload = vi.mocked(pauseDownload);
const mockResumeDownload = vi.mocked(resumeDownload);
const mockCancelDownload = vi.mocked(cancelDownload);
const mockGetDownloadStatus = vi.mocked(getDownloadStatus);
const mockListDownloads = vi.mocked(listDownloads);
const mockGetBtRuntimeStatus = vi.mocked(getBtRuntimeStatus);
const mockSetPriority = vi.mocked(setPriority);

function mockBtStatus(overrides?: Partial<BtRuntimeStatus>): BtRuntimeStatus {
  return {
    connected: true,
    dhtEnabled: true,
    dhtNodes: null,
    torrentCount: 3,
    peerCount: 42,
    uploadSpeedBytesPerSecond: null,
    uploadedBytes: 0,
    updatedAtMs: Date.now(),
    seedCount: null,
    leechCount: null,
    ...overrides,
  };
}

describe("useDownloadStore (extended)", () => {
  let store: ReturnType<typeof useDownloadStore>;

  beforeEach(() => {
    resetMockIds();
    setActivePinia(createPinia());
    store = useDownloadStore();
  });

  afterEach(() => {
    vi.clearAllMocks();
    store.destroyStore();
  });

  // ── A. Lifecycle (configure / init / destroy) ──────────────────

  describe("lifecycle", () => {
    it("configure stores callbacks and onDownloadsRemoved fires on remove", async () => {
      const onRemoved = vi.fn();
      store.configure({ onDownloadsRemoved: onRemoved });

      const task = DownloadPresets.downloading({ id: "t-1", fileName: "alpha.zip" });
      store.downloads = [task];
      store.selectedId = "t-1";
      const snapshot = createMockDownloadSnapshot({ id: "t-1", state: "canceled" });
      mockCancelDownload.mockResolvedValue(snapshot);

      await store.runCancel();

      expect(onRemoved).toHaveBeenCalledTimes(1);
      expect(onRemoved).toHaveBeenCalledWith(["t-1"]);
    });

    it("initStore calls listDownloads and getBtRuntimeStatus silently", async () => {
      mockListDownloads.mockResolvedValue([]);
      mockGetBtRuntimeStatus.mockResolvedValue(mockBtStatus());

      await store.initStore();

      expect(mockListDownloads).toHaveBeenCalledTimes(1);
      expect(mockGetBtRuntimeStatus).toHaveBeenCalledTimes(1);
    });

    it("initStore calls refreshStatus for selectedId when present", async () => {
      const task = DownloadPresets.downloading({ id: "t-1" });
      store.downloads = [task];
      store.selectedId = "t-1";
      const snapshot = createMockDownloadSnapshot({ id: "t-1" });
      mockListDownloads.mockResolvedValue([task]);
      mockGetBtRuntimeStatus.mockResolvedValue(mockBtStatus());
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.initStore();

      expect(mockGetDownloadStatus).toHaveBeenCalledWith("t-1");
    });

    it("destroyStore cleans up without errors", async () => {
      mockListDownloads.mockResolvedValue([]);
      mockGetBtRuntimeStatus.mockResolvedValue(mockBtStatus());
      await store.initStore();

      expect(() => store.destroyStore()).not.toThrow();
    });
  });

  // ── B. Derived helpers (can*) ──────────────────────────────────

  describe("derived helpers", () => {
    it("canPause returns true when selected download is downloading", () => {
      store.selectedId = "t-1";
      store.selectedSnapshot = createMockDownloadSnapshot({ id: "t-1", state: "downloading" });
      expect(store.canPause).toBe(true);
    });

    it("canPause returns false when selected download is paused", () => {
      store.selectedId = "t-1";
      store.selectedSnapshot = createMockDownloadSnapshot({ id: "t-1", state: "paused" });
      expect(store.canPause).toBe(false);
    });

    it("canPause returns false when no download is selected", () => {
      expect(store.canPause).toBe(false);
    });

    it("canResume returns true when selected download is paused", () => {
      store.selectedId = "t-1";
      store.selectedSnapshot = createMockDownloadSnapshot({ id: "t-1", state: "paused" });
      expect(store.canResume).toBe(true);
    });

    it("canResume returns false when selected download is downloading", () => {
      store.selectedId = "t-1";
      store.selectedSnapshot = createMockDownloadSnapshot({ id: "t-1", state: "downloading" });
      expect(store.canResume).toBe(false);
    });

    it("canResume returns false when no download is selected", () => {
      expect(store.canResume).toBe(false);
    });

    it.each<[string, "downloading" | "completed" | "failed", boolean]>([
      ["canCancel returns true for active (downloading) state", "downloading", true],
      ["canCancel returns false for terminal (completed) state", "completed", false],
      ["canCancel returns false for terminal (failed) state", "failed", false],
    ])("%s", (_title, state, expected) => {
      store.selectedId = "t-1";
      store.selectedSnapshot = createMockDownloadSnapshot({ id: "t-1", state });
      expect(store.canCancel).toBe(expected);
    });

    it("canCancel returns false when no download is selected", () => {
      expect(store.canCancel).toBe(false);
    });

    it("canPauseDownload checks individual download state (downloading -> true)", () => {
      const task = DownloadPresets.downloading({ id: "t-1" });
      expect(store.canPauseDownload(task)).toBe(true);
    });

    it("canPauseDownload checks individual download state (paused -> false)", () => {
      const task = DownloadPresets.paused({ id: "t-1" });
      expect(store.canPauseDownload(task)).toBe(false);
    });

    it("canResumeDownload checks individual download state (paused -> true)", () => {
      const task = DownloadPresets.paused({ id: "t-1" });
      expect(store.canResumeDownload(task)).toBe(true);
    });

    it("canResumeDownload checks individual download state (downloading -> false)", () => {
      const task = DownloadPresets.downloading({ id: "t-1" });
      expect(store.canResumeDownload(task)).toBe(false);
    });

    it("canResumeDownload returns true for failed state", () => {
      const task = DownloadPresets.failed({ id: "t-1" });
      expect(store.canResumeDownload(task)).toBe(true);
    });
  });

  // ── C. Single actions (runPauseFor / runResumeFor) ─────────────

  describe("single actions", () => {
    it("runPauseFor pauses a specific download and updates summary", async () => {
      const task = DownloadPresets.downloading({ id: "t-1", fileName: "alpha.zip" });
      store.downloads = [task];
      const snapshot = createMockDownloadSnapshot({
        id: "t-1",
        state: "paused",
        fileName: "alpha.zip",
      });
      mockPauseDownload.mockResolvedValue(snapshot);

      await store.runPauseFor("t-1");

      expect(mockPauseDownload).toHaveBeenCalledWith("t-1");
      expect(store.downloads[0].state).toBe("paused");
    });

    it("runPauseFor sets actionName during and resets after", async () => {
      const task = DownloadPresets.downloading({ id: "t-1" });
      store.downloads = [task];
      let resolveSnapshot!: (s: ReturnType<typeof createMockDownloadSnapshot>) => void;
      const pending = new Promise<ReturnType<typeof createMockDownloadSnapshot>>((resolve) => {
        resolveSnapshot = resolve;
      });
      mockPauseDownload.mockReturnValue(pending);

      const promise = store.runPauseFor("t-1");
      expect(store.actionName).toBe("Pause");

      resolveSnapshot(createMockDownloadSnapshot({ id: "t-1", state: "paused" }));
      await promise;
      expect(store.actionName).toBe("");
    });

    it("runPauseFor handles error and resets actionName", async () => {
      const task = DownloadPresets.downloading({ id: "t-1" });
      store.downloads = [task];
      mockPauseDownload.mockRejectedValue(new Error("Backend error"));

      await store.runPauseFor("t-1");

      expect(store.actionName).toBe("");
    });

    it("runResumeFor resumes a specific download and updates summary", async () => {
      const task = DownloadPresets.paused({ id: "t-1", fileName: "beta.zip" });
      store.downloads = [task];
      const snapshot = createMockDownloadSnapshot({
        id: "t-1",
        state: "downloading",
        fileName: "beta.zip",
      });
      mockResumeDownload.mockResolvedValue(snapshot);

      await store.runResumeFor("t-1");

      expect(mockResumeDownload).toHaveBeenCalledWith("t-1");
      expect(store.downloads[0].state).toBe("downloading");
    });
  });

  // ── D. Batch actions ───────────────────────────────────────────

  describe("batch actions", () => {
    it("runBatchPause pauses only pausable downloads", async () => {
      const d1 = DownloadPresets.downloading({ id: "t-1", fileName: "a.zip" });
      const d2 = DownloadPresets.paused({ id: "t-2", fileName: "b.zip" });
      const d3 = DownloadPresets.downloading({ id: "t-3", fileName: "c.zip" });
      store.downloads = [d1, d2, d3];

      const snap1 = createMockDownloadSnapshot({ id: "t-1", state: "paused" });
      const snap3 = createMockDownloadSnapshot({ id: "t-3", state: "paused" });
      mockPauseDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap3);

      await store.runBatchPause(["t-1", "t-2", "t-3"]);

      expect(mockPauseDownload).toHaveBeenCalledTimes(2);
      expect(mockPauseDownload).toHaveBeenCalledWith("t-1");
      expect(mockPauseDownload).toHaveBeenCalledWith("t-3");
      expect(store.actionName).toBe("");
    });

    it("runBatchPause returns early on empty array", async () => {
      await store.runBatchPause([]);
      expect(mockPauseDownload).not.toHaveBeenCalled();
    });

    it("runBatchResume resumes only resumable downloads", async () => {
      const d1 = DownloadPresets.paused({ id: "t-1", fileName: "a.zip" });
      const d2 = DownloadPresets.downloading({ id: "t-2", fileName: "b.zip" });
      const d3 = DownloadPresets.failed({ id: "t-3", fileName: "c.zip" });
      store.downloads = [d1, d2, d3];

      const snap1 = createMockDownloadSnapshot({ id: "t-1", state: "downloading" });
      const snap3 = createMockDownloadSnapshot({ id: "t-3", state: "downloading" });
      mockResumeDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap3);

      await store.runBatchResume(["t-1", "t-2", "t-3"]);

      expect(mockResumeDownload).toHaveBeenCalledTimes(2);
      expect(mockResumeDownload).toHaveBeenCalledWith("t-1");
      expect(mockResumeDownload).toHaveBeenCalledWith("t-3");
    });

    it("runBatchResume returns early on empty array", async () => {
      await store.runBatchResume([]);
      expect(mockResumeDownload).not.toHaveBeenCalled();
    });

    it("runBatchCancel cancels only non-terminal downloads", async () => {
      const d1 = DownloadPresets.downloading({ id: "t-1", fileName: "a.zip" });
      const d2 = DownloadPresets.completed({ id: "t-2", fileName: "b.zip" });
      const d3 = DownloadPresets.queued({ id: "t-3", fileName: "c.zip" });
      store.downloads = [d1, d2, d3];

      const snap1 = createMockDownloadSnapshot({ id: "t-1", state: "canceled" });
      const snap3 = createMockDownloadSnapshot({ id: "t-3", state: "canceled" });
      mockCancelDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap3);

      await store.runBatchCancel(["t-1", "t-2", "t-3"]);

      expect(mockCancelDownload).toHaveBeenCalledTimes(2);
      expect(mockCancelDownload).toHaveBeenCalledWith("t-1");
      expect(mockCancelDownload).toHaveBeenCalledWith("t-3");
    });

    it("runBatchCancel removes cancelled downloads from list", async () => {
      const d1 = DownloadPresets.downloading({ id: "t-1", fileName: "a.zip" });
      const d3 = DownloadPresets.queued({ id: "t-3", fileName: "c.zip" });
      store.downloads = [d1, d3];

      const snap1 = createMockDownloadSnapshot({ id: "t-1", state: "canceled" });
      const snap3 = createMockDownloadSnapshot({ id: "t-3", state: "canceled" });
      mockCancelDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap3);

      await store.runBatchCancel(["t-1", "t-3"]);

      expect(store.downloads).toHaveLength(0);
    });

    it("runBatchCancel returns early on empty array", async () => {
      await store.runBatchCancel([]);
      expect(mockCancelDownload).not.toHaveBeenCalled();
    });

    it("runSetPriority calls setPriority and updates local summary", async () => {
      const task = DownloadPresets.downloading({
        id: "t-1",
        fileName: "a.zip",
        priority: "normal",
      });
      store.downloads = [task];

      mockSetPriority.mockResolvedValue(undefined);
      await store.runSetPriority("t-1", "high");

      expect(mockSetPriority).toHaveBeenCalledWith("t-1", "high");
      expect(store.downloads[0].priority).toBe("high");
    });

    it("runSetPriority handles error gracefully", async () => {
      mockSetPriority.mockRejectedValue(new Error("Backend error"));
      await expect(store.runSetPriority("t-1", "low")).resolves.toBeUndefined();
    });
  });

  // ── E. State setters ───────────────────────────────────────────

  describe("state setters", () => {
    it("setMessage calls notification store notifyInfo", () => {
      expect(() => store.setMessage("hello")).not.toThrow();
    });

    it("setError calls notification store notifyError", () => {
      expect(() => store.setError("oops")).not.toThrow();
    });

    it("setNotificationsEnabled can be called without error", () => {
      expect(() => store.setNotificationsEnabled(true)).not.toThrow();
      expect(() => store.setNotificationsEnabled(false)).not.toThrow();
    });
  });

  // ── F. Form helpers ────────────────────────────────────────────

  describe("form helpers", () => {
    it("applySchedulerDefaults with automatic mode sets threadMode to adaptive", () => {
      store.form.kind = "http";
      store.form.threadMode = "fixed";
      store.form.threadCount = 8;

      store.applySchedulerDefaults("automatic", 4);

      expect(store.form.threadMode).toBe("adaptive");
      expect(store.form.threadCount).toBe(4);
    });

    it("applySchedulerDefaults with automatic mode does not increase threadCount", () => {
      store.form.kind = "http";
      store.form.threadMode = "adaptive";
      store.form.threadCount = 2;

      store.applySchedulerDefaults("automatic", 4);

      expect(store.form.threadCount).toBe(2);
    });

    it("applySchedulerDefaults with traditional mode sets threadMode to fixed", () => {
      store.form.kind = "http";
      store.form.threadMode = "adaptive";

      store.applySchedulerDefaults("traditional", 4);

      expect(store.form.threadMode).toBe("fixed");
      expect(store.form.threadCount).toBe(4);
    });

    it("applySchedulerDefaults with traditional mode defaults threadCount to 8 when unset", () => {
      store.form.kind = "http";
      store.form.threadMode = "adaptive";
      store.form.threadCount = 0;

      store.applySchedulerDefaults("traditional", undefined);

      expect(store.form.threadMode).toBe("fixed");
      expect(store.form.threadCount).toBe(8);
    });

    it("applySchedulerDefaults returns early for non-http kind", () => {
      store.form.kind = "bt";
      store.form.threadMode = "adaptive";

      store.applySchedulerDefaults("traditional", 2);

      expect(store.form.threadMode).toBe("adaptive");
    });

    it("applyAppSettingsDefaults applies download defaults to form", () => {
      const settings = createMinimalAppSettings({
        download: {
          defaultDownloadDir: "C:\\Downloads",
          defaultMaxRetries: 5,
          defaultChecksum: "sha256" as const,
          defaultUserAgent: "TestAgent/1.0",
        },
        scheduler: {
          mode: "automatic" as const,
          automatic: {
            maxParallelThreads: 4,
            maxThreadsPerTask: 6,
            minThreadsPerTask: 1,
            adaptiveProfile: "balanced" as const,
          },
          traditional: { maxParallelTasks: 3 },
          chunkSizeStrategy: "adaptive" as const,
          tailSprintEnabled: false,
          connectionWarmupEnabled: false,
        },
      });

      store.form.kind = "http";
      store.form.threadMode = "fixed";
      store.form.threadCount = 10;

      store.applyAppSettingsDefaults(settings);

      expect(store.form.destinationDir).toBe("C:\\Downloads");
      expect(store.form.maxRetries).toBe(5);
      expect(store.form.checksum).toBe("sha256");
      expect(store.form.userAgent).toBe("TestAgent/1.0");
    });
  });

  // ── G. Refresh actions ─────────────────────────────────────────

  describe("refresh actions", () => {
    it("refreshList fetches full download list and populates downloads ref", async () => {
      const mockData = [
        createMockDownloadTask({ id: "t-1", fileName: "a.zip" }),
        createMockDownloadTask({ id: "t-2", fileName: "b.zip" }),
      ];
      mockListDownloads.mockResolvedValue(mockData);

      await store.refreshList();

      expect(mockListDownloads).toHaveBeenCalledTimes(1);
      expect(store.downloads).toHaveLength(2);
      expect(store.downloads[0].id).toBe("t-1");
      expect(store.downloads[1].fileName).toBe("b.zip");
    });

    it("refreshList handles empty list gracefully", async () => {
      mockListDownloads.mockResolvedValue([]);

      await store.refreshList();

      expect(store.downloads).toEqual([]);
    });

    it("refreshList sets isRefreshingList guard", async () => {
      let resolvePromise!: (v: DownloadSummary[]) => void;
      const pending = new Promise<DownloadSummary[]>((resolve) => {
        resolvePromise = resolve;
      });
      mockListDownloads.mockReturnValue(pending);

      const promise = store.refreshList();
      expect(store.isRefreshingList).toBe(true);

      resolvePromise([]);
      await promise;
      expect(store.isRefreshingList).toBe(false);
    });

    it("refreshList guards against concurrent calls", async () => {
      let resolvePromise!: (v: DownloadSummary[]) => void;
      const pending = new Promise<DownloadSummary[]>((resolve) => {
        resolvePromise = resolve;
      });
      mockListDownloads.mockReturnValue(pending);

      const r1 = store.refreshList();
      const r2 = store.refreshList(); // should be a no-op

      resolvePromise([]);
      await r1;
      await r2;

      expect(mockListDownloads).toHaveBeenCalledTimes(1);
    });

    it("refreshBtRuntimeStatus fetches BT runtime status", async () => {
      const btStatus = mockBtStatus({
        torrentCount: 3,
        peerCount: 42,
        dhtNodes: 8,
      });
      mockGetBtRuntimeStatus.mockResolvedValue(btStatus);

      await store.refreshBtRuntimeStatus();

      expect(mockGetBtRuntimeStatus).toHaveBeenCalledTimes(1);
      expect(store.btRuntimeStatus).toEqual(btStatus);
    });

    it("refreshBtRuntimeStatus handles error silently with silent option", async () => {
      mockGetBtRuntimeStatus.mockRejectedValue(new Error("BT backend offline"));

      await store.refreshBtRuntimeStatus({ silent: true });
      expect(store.btRuntimeStatus).toBeNull();
    });

    it("refreshStatus fetches status for a specific download ID", async () => {
      const snapshot = createMockDownloadSnapshot({ id: "t-1", fileName: "test.zip" });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.refreshStatus("t-1");

      expect(mockGetDownloadStatus).toHaveBeenCalledWith("t-1");
    });

    it("refreshStatus updates selectedSnapshot when id matches selectedId", async () => {
      store.selectedId = "t-1";
      const snapshot = createMockDownloadSnapshot({ id: "t-1", fileName: "test.zip" });
      mockGetDownloadStatus.mockResolvedValue(snapshot);

      await store.refreshStatus("t-1");

      expect(store.selectedSnapshot?.id).toBe("t-1");
    });

    it("refreshStatus does nothing when downloadId is falsy", async () => {
      await store.refreshStatus("");
      expect(mockGetDownloadStatus).not.toHaveBeenCalled();
    });

    it("refreshStatus guards against concurrent calls", async () => {
      let resolvePromise!: (v: DownloadSnapshot) => void;
      const pending = new Promise<DownloadSnapshot>((resolve) => {
        resolvePromise = resolve;
      });
      mockGetDownloadStatus.mockReturnValue(pending);

      const r1 = store.refreshStatus("t-1");
      const r2 = store.refreshStatus("t-1"); // should be a no-op

      resolvePromise(createMockDownloadSnapshot({ id: "t-1" }));
      await r1;
      await r2;

      expect(mockGetDownloadStatus).toHaveBeenCalledTimes(1);
    });
  });

  // ── H. Batch mode / form ───────────────────────────────────────

  describe("batch mode and form", () => {
    it("toggleBatchMode enables batch mode", () => {
      expect(store.batchMode).toBe(false);
      store.toggleBatchMode();
      expect(store.batchMode).toBe(true);
    });

    it("toggleBatchMode disables batch mode and clears batch form", () => {
      store.batchMode = true;
      store.batchUrls = "https://example.com/file1.zip\nhttps://example.com/file2.zip";
      store.batchEntries = [
        {
          id: "e-1",
          url: "https://example.com/file1.zip",
          kind: "http",
          fileName: "file1.zip",
          status: "ready",
        },
      ];
      store.batchSubmitProgress = { done: 1, total: 2 };

      store.toggleBatchMode();

      expect(store.batchMode).toBe(false);
      expect(store.batchUrls).toBe("");
      expect(store.batchEntries).toEqual([]);
      expect(store.batchSubmitProgress).toEqual({ done: 0, total: 0 });
    });

    it("parseBatchUrls parses newline-separated URLs into entries", () => {
      store.batchUrls = "https://example.com/a.zip\nhttps://example.com/b.zip";
      store.parseBatchUrls();

      expect(store.batchEntries).toHaveLength(2);
      expect(store.batchEntries[0].url).toBe("https://example.com/a.zip");
      expect(store.batchEntries[0].kind).toBe("http");
      expect(store.batchEntries[0].status).toBe("ready");
      expect(store.batchEntries[1].url).toBe("https://example.com/b.zip");
    });

    it("parseBatchUrls skips empty lines and comments", () => {
      store.batchUrls =
        "https://example.com/a.zip\n  \n# this is a comment\nhttps://example.com/b.zip";
      store.parseBatchUrls();

      expect(store.batchEntries).toHaveLength(2);
    });

    it("parseBatchUrls expands URL range patterns", () => {
      store.batchUrls = "https://example.com/file[1-3].zip";
      store.parseBatchUrls();

      expect(store.batchEntries).toHaveLength(3);
      expect(store.batchEntries[0].url).toBe("https://example.com/file1.zip");
      expect(store.batchEntries[1].url).toBe("https://example.com/file2.zip");
      expect(store.batchEntries[2].url).toBe("https://example.com/file3.zip");
    });

    it("parseBatchUrls handles empty batchUrls", () => {
      store.batchUrls = "";
      store.parseBatchUrls();
      expect(store.batchEntries).toEqual([]);
    });

    it("batchUrls getter/setter works correctly", () => {
      store.batchUrls = "https://example.com/test.zip";
      expect(store.batchUrls).toBe("https://example.com/test.zip");
    });
  });
});
