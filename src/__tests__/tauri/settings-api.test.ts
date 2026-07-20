import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

import type { AppSettings } from "../../types/settings";
import { invoke } from "#invoke";
import { createMockInvoke, resetTauriMocks, mockTauriCommandValue } from "../mocks/tauri-mock";
import {
  getAppSettings,
  saveAppSettings,
  fetchTrackerList,
  toggleGameMode,
  getIoStatus,
  toggleOverclockMode,
  getOverclockMode,
} from "../../lib/tauri/settings-api";

const mockInvoke = vi.mocked(invoke);

function makeAppSettings(overrides?: Partial<AppSettings>): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "amber",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: false,
      showHeatmap: false,
      sortKey: "name",
      sortDirection: "asc",
      compactView: false,
      visibleColumns: [],
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
      chunkSizeStrategy: "fixed",
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 3,
      defaultChecksum: "none",
      defaultUserAgent: "",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 2,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "",
      listenPort: null,
      listenPortRange: null,
      upnpEnabled: false,
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
      maxDownloads: 3,
      maxSeeds: 1,
      maxTorrents: 10,
      activeLimit: 10,
    },
    logging: {
      enabled: false,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: false, port: 6800, secret: null, corsAllowedOrigins: [], },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: {
      bufferLimitMb: 256,
      gameModeBufferMb: 64,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 2,
      gameModeMaxParallel: 1,
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    ...overrides,
  };
}

beforeEach(() => {
  resetTauriMocks();
  mockInvoke.mockImplementation(createMockInvoke());
  vi.clearAllMocks();
});

describe("settings-api", () => {
  it("getAppSettings calls settings_get", async () => {
    const settings = makeAppSettings({
      download: { ...makeAppSettings().download, defaultDownloadDir: "/downloads" },
    });
    mockTauriCommandValue("settings_get", settings);

    const result = await getAppSettings();

    expect(mockInvoke).toHaveBeenCalledWith("settings_get");
    expect(result).toEqual(settings);
  });

  it("saveAppSettings calls settings_save with settings", async () => {
    const settings = makeAppSettings({
      scheduler: { ...makeAppSettings().scheduler, traditional: { maxParallelTasks: 5 } },
    });
    mockTauriCommandValue("settings_save", settings);

    const result = await saveAppSettings(settings);

    expect(mockInvoke).toHaveBeenCalledWith("settings_save", { settings });
    expect(result).toEqual(settings);
  });

  it("fetchTrackerList calls settings_fetch_tracker_list with trackerListUrl", async () => {
    const url = "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt";
    const content = "udp://tracker.example.com:6969\n";
    mockTauriCommandValue("settings_fetch_tracker_list", content);

    const result = await fetchTrackerList(url);

    expect(mockInvoke).toHaveBeenCalledWith("settings_fetch_tracker_list", {
      trackerListUrl: url,
    });
    expect(result).toBe(content);
  });

  it("toggleGameMode calls toggle_game_mode with enabled", async () => {
    mockTauriCommandValue("toggle_game_mode", true);

    const result = await toggleGameMode(true);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_game_mode", { enabled: true });
    expect(result).toBe(true);
  });

  it("toggleGameMode calls toggle_game_mode with enabled false", async () => {
    mockTauriCommandValue("toggle_game_mode", false);

    const result = await toggleGameMode(false);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_game_mode", { enabled: false });
    expect(result).toBe(false);
  });

  it("getIoStatus calls get_io_status", async () => {
    const ioStatus = {
      gameMode: true,
      bufferUsageBytes: 1048576,
      bufferLimitBytes: 8388608,
      degradationCount: 0,
      activeSlots: 2,
      maxSlots: 4,
      queuedCount: 1,
    };
    mockTauriCommandValue("get_io_status", ioStatus);

    const result = await getIoStatus();

    expect(mockInvoke).toHaveBeenCalledWith("get_io_status");
    expect(result).toEqual(ioStatus);
  });

  it("toggleOverclockMode calls toggle_overclock_mode with enabled", async () => {
    mockTauriCommandValue("toggle_overclock_mode", true);

    const result = await toggleOverclockMode(true);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_overclock_mode", { enabled: true });
    expect(result).toBe(true);
  });

  it("toggleOverclockMode calls toggle_overclock_mode with enabled false", async () => {
    mockTauriCommandValue("toggle_overclock_mode", false);

    const result = await toggleOverclockMode(false);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_overclock_mode", { enabled: false });
    expect(result).toBe(false);
  });

  it("getOverclockMode calls get_overclock_mode", async () => {
    mockTauriCommandValue("get_overclock_mode", true);

    const result = await getOverclockMode();

    expect(mockInvoke).toHaveBeenCalledWith("get_overclock_mode");
    expect(result).toBe(true);
  });
});
