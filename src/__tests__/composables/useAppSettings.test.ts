import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { nextTick } from "vue";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../i18n", () => ({
  t: vi.fn((key: string) => key),
}));

vi.mock("../../lib/tauri/settings-api", () => ({
  getAppSettings: vi.fn(),
  saveAppSettings: vi.fn().mockResolvedValue(undefined),
}));

import { invoke } from "@tauri-apps/api/core";
import { createMockInvoke, resetTauriMocks } from "../mocks/tauri-mock";
import { getAppSettings, saveAppSettings } from "../../lib/tauri/settings-api";
import { useAppSettingsStore } from "../../stores/appSettings";
import { useDownloadStore } from "../../stores/download";
import { DEFAULT_VISIBLE_COLUMNS } from "../../lib/column-defs";
import type { AppSettings, SortKey } from "../../types/settings";

const mockInvoke = vi.mocked(invoke);
const mockGetAppSettings = vi.mocked(getAppSettings);
const mockSaveAppSettings = vi.mocked(saveAppSettings);

// Mock the notification store inside download store
vi.mock("../../stores/notification", () => ({
  useNotificationStore: () => ({
    notifySuccess: vi.fn(),
    notifyError: vi.fn(),
    notifyInfo: vi.fn(),
    notifyWarning: vi.fn(),
    clearAll: vi.fn(),
    notify: vi.fn(),
    dismiss: vi.fn(),
    notifications: { value: [] },
  }),
}));

function createDefaultSettings(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "lime",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: false,
      showHeatmap: false,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: [...DEFAULT_VISIBLE_COLUMNS],
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

function settingsWithAppearance(overrides: Partial<AppSettings["appearance"]> = {}): AppSettings {
  const base = createDefaultSettings();
  base.appearance = { ...base.appearance, ...overrides };
  return base;
}

describe("useAppSettingsStore", () => {
  let store: ReturnType<typeof useAppSettingsStore>;
  let downloadStore: ReturnType<typeof useDownloadStore>;
  let matchMediaMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    resetTauriMocks();
    mockInvoke.mockImplementation(createMockInvoke());

    vi.spyOn(console, "error").mockImplementation(() => {});

    delete document.documentElement.dataset.colorMode;
    delete document.documentElement.dataset.colorModePreference;
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.surface;

    matchMediaMock = vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      value: matchMediaMock,
    });

    setActivePinia(createPinia());
    store = useAppSettingsStore();
    downloadStore = useDownloadStore();
  });

  afterEach(() => {
    vi.resetAllMocks();
    store.destroyStore();
  });

  // ── applyColorMode (via applyAppearanceSettings) ─────────────────

  describe("applyColorMode", () => {
    it('sets data-color-mode="dark" for dark mode', () => {
      store.applyAppearanceSettings(settingsWithAppearance({ colorMode: "dark" }));

      expect(document.documentElement.dataset.colorModePreference).toBe("dark");
      expect(document.documentElement.dataset.colorMode).toBe("dark");
    });

    it('sets data-color-mode="light" for light mode', () => {
      store.applyAppearanceSettings(settingsWithAppearance({ colorMode: "light" }));

      expect(document.documentElement.dataset.colorModePreference).toBe("light");
      expect(document.documentElement.dataset.colorMode).toBe("light");
    });

    it("resolves system to dark when prefers-color-scheme: dark", () => {
      matchMediaMock.mockImplementation((query: string) => ({
        matches: query === "(prefers-color-scheme: dark)",
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }));

      store.applyAppearanceSettings(settingsWithAppearance({ colorMode: "system" }));

      expect(document.documentElement.dataset.colorModePreference).toBe("system");
      expect(document.documentElement.dataset.colorMode).toBe("dark");
    });
  });

  // ── applyAppearanceSettings ─────────────────────────────────────

  describe("applyAppearanceSettings", () => {
    it("sets data-theme and data-surface attributes", () => {
      store.applyAppearanceSettings(
        settingsWithAppearance({
          themeColor: "sky",
          backgroundOpacity: "frosted",
        }),
      );

      expect(document.documentElement.dataset.theme).toBe("sky");
      expect(document.documentElement.dataset.surface).toBe("frosted");
    });

    it("falls back to defaults when appearance fields are missing", () => {
      const settings = createDefaultSettings();
      Object.assign(settings.appearance, {
        themeColor: undefined,
        backgroundOpacity: undefined,
        colorMode: undefined,
      });
      store.applyAppearanceSettings(settings);

      expect(document.documentElement.dataset.theme).toBe("lime");
      expect(document.documentElement.dataset.surface).toBe("default");
      expect(document.documentElement.dataset.colorModePreference).toBe("system");
      expect(document.documentElement.dataset.colorMode).toBe("light");
    });
  });

  // ── loadSettings (via initStore) ────────────────────────────────

  describe("loadSettings", () => {
    it("loads settings via getAppSettings and stores in appSettings ref", async () => {
      const settings = createDefaultSettings();
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(mockGetAppSettings).toHaveBeenCalledTimes(1);
      expect(store.appSettings).toEqual(settings);
    });

    it("handles API error gracefully", async () => {
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      mockGetAppSettings.mockRejectedValue(new Error("Network error"));

      store.initStore();
      await nextTick();

      expect(store.appSettings).toBeNull();
      expect(consoleSpy).toHaveBeenCalledWith("Failed to load app settings", expect.any(Error));
      consoleSpy.mockRestore();
    });

    it("calls downloadStore.applyAppSettingsDefaults with loaded settings", async () => {
      vi.spyOn(downloadStore, "applyAppSettingsDefaults");
      const settings = createDefaultSettings();
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(downloadStore.applyAppSettingsDefaults).toHaveBeenCalledTimes(1);
      expect(downloadStore.applyAppSettingsDefaults).toHaveBeenCalledWith(settings);
    });

    it("applies color mode from loaded settings", async () => {
      const settings = settingsWithAppearance({ colorMode: "dark" });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(document.documentElement.dataset.colorModePreference).toBe("dark");
      expect(document.documentElement.dataset.colorMode).toBe("dark");
    });
  });

  // ── Settings save (debounced) ───────────────────────────────────

  describe("settings save (debounced)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    /** Create store with appSettings loaded directly */
    function createLoaded() {
      mockGetAppSettings.mockReturnValue(new Promise<AppSettings>(() => {}));
      store.initStore();
      store.appSettings = createDefaultSettings();
    }

    it("triggers debounced saveAppSettings when sortKey changes", async () => {
      createLoaded();
      // Flush Vue queue so watchers fire
      await nextTick();
      await nextTick();

      store.sortKey = "name" as SortKey;
      await nextTick();

      // Should not be saved immediately (debounced)
      expect(mockSaveAppSettings).not.toHaveBeenCalled();

      // Advance past debounce delay
      await vi.advanceTimersByTimeAsync(300);

      expect(mockSaveAppSettings).toHaveBeenCalledTimes(1);
      expect(mockSaveAppSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          appearance: expect.objectContaining({ sortKey: "name" }),
        }),
      );
    });

    it("debounces multiple rapid changes into a single save call", async () => {
      createLoaded();
      await nextTick();
      await nextTick();

      store.sortKey = "name" as SortKey;
      await nextTick();
      store.sortKey = "size" as SortKey;
      await nextTick();
      store.sortKey = "added_at" as SortKey;
      await nextTick();

      await vi.advanceTimersByTimeAsync(300);

      expect(mockSaveAppSettings).toHaveBeenCalledTimes(1);
      expect(mockSaveAppSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          appearance: expect.objectContaining({ sortKey: "added_at" }),
        }),
      );
    });

    it("does not save when appSettings is null (not yet loaded)", async () => {
      mockGetAppSettings.mockReturnValue(new Promise<AppSettings>(() => {}));
      store.initStore();
      await nextTick();

      store.sortKey = "name" as SortKey;
      await nextTick();
      await vi.advanceTimersByTimeAsync(300);

      expect(mockSaveAppSettings).not.toHaveBeenCalled();
    });

    it("saves updated compactView value", async () => {
      createLoaded();
      await nextTick();
      await nextTick();

      store.compactView = true;
      await nextTick();
      await vi.advanceTimersByTimeAsync(300);

      expect(mockSaveAppSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          appearance: expect.objectContaining({ compactView: true }),
        }),
      );
    });
  });

  // ── Column settings ─────────────────────────────────────────────

  describe("column settings", () => {
    it("loads visible columns from settings into visibleColumns ref", async () => {
      const cols = ["file", "size", "status", "progress", "speed", "eta"];
      const settings = settingsWithAppearance({ visibleColumns: cols });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();
      await nextTick();

      expect(store.visibleColumns).toEqual(cols);
    });

    it("filters out invalid column keys", async () => {
      const settings = settingsWithAppearance({
        visibleColumns: ["file", "size", "invalid_key", "status"],
      });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();
      await nextTick();

      expect(store.visibleColumns).toEqual(["file", "size", "status"]);
    });

    it("ensures 'file' column is always first", async () => {
      const settings = settingsWithAppearance({
        visibleColumns: ["size", "status", "file"],
      });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(store.visibleColumns[0]).toBe("file");
      expect(store.visibleColumns).toContain("size");
      expect(store.visibleColumns).toContain("status");
    });
  });

  // ── Sort settings ───────────────────────────────────────────────

  describe("sort settings", () => {
    it("loads sort key and direction from settings", async () => {
      const settings = settingsWithAppearance({
        sortKey: "name",
        sortDirection: "asc",
      });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();
      await nextTick();

      expect(store.sortKey).toBe("name");
      expect(store.sortDirection).toBe("asc");
    });

    it("uses default sort key 'added_at' when settings value is null", async () => {
      const nullOverride: any = null;
      const settings = settingsWithAppearance({ sortKey: nullOverride });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(store.sortKey).toBe("added_at");
    });

    it("uses default sort direction 'desc' when settings value is null", async () => {
      const nullOverride: any = null;
      const settings = settingsWithAppearance({
        sortDirection: nullOverride,
      });
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();

      expect(store.sortDirection).toBe("desc");
    });
  });

  // ── Notifications sync ─────────────────────────────────────────

  describe("notifications sync", () => {
    it("calls downloadStore.setNotificationsEnabled with loaded settings value", async () => {
      vi.spyOn(downloadStore, "setNotificationsEnabled");
      const settings = createDefaultSettings();
      settings.notifications.enabled = true;
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();
      await nextTick();

      expect(downloadStore.setNotificationsEnabled).toHaveBeenCalledWith(true);
    });

    it("calls setNotificationsEnabled with false when settings has disabled notifications", async () => {
      vi.spyOn(downloadStore, "setNotificationsEnabled");
      const settings = createDefaultSettings();
      settings.notifications.enabled = false;
      mockGetAppSettings.mockResolvedValue(settings);

      store.initStore();
      await nextTick();
      await nextTick();

      expect(downloadStore.setNotificationsEnabled).toHaveBeenCalledWith(false);
    });
  });
});
