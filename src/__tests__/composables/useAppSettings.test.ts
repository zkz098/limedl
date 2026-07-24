import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { ref, nextTick, type Ref } from "vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../i18n", () => ({
  t: vi.fn((key: string) => key),
}));

// Mock Vue lifecycle hooks so onMounted runs immediately
vi.mock("vue", async () => {
  const actual = await vi.importActual("vue");
  return {
    ...actual,
    onMounted: vi.fn((cb: () => void) => cb()),
    onBeforeUnmount: vi.fn(),
  };
});

vi.mock("../../lib/tauri/settings-api", () => ({
  getAppSettings: vi.fn(),
  saveAppSettings: vi.fn().mockResolvedValue(undefined),
}));

import { invoke } from "@tauri-apps/api/core";
import { createMockInvoke, resetTauriMocks } from "../mocks/tauri-mock";
import { getAppSettings, saveAppSettings } from "../../lib/tauri/settings-api";
import { useAppSettings, type UseAppSettingsParams } from "../../composables/useAppSettings";
import { DEFAULT_VISIBLE_COLUMNS } from "../../lib/column-defs";
import type { AppSettings, SortKey, SortDirection } from "../../types/settings";

const mockInvoke = vi.mocked(invoke);
const mockGetAppSettings = vi.mocked(getAppSettings);
const mockSaveAppSettings = vi.mocked(saveAppSettings);

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
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    ...overrides,
  };
}

/**
 * Convenience helper: returns a full AppSettings with only the `appearance`
 * fields overridden from defaults.
 */
function settingsWithAppearance(overrides: Partial<AppSettings["appearance"]> = {}): AppSettings {
  const base = createDefaultSettings();
  base.appearance = { ...base.appearance, ...overrides };
  return base;
}

describe("useAppSettings", () => {
  let sortKey: Ref<SortKey>;
  let sortDirection: Ref<SortDirection>;
  let compactView: Ref<boolean>;
  let visibleColumns: Ref<string[]>;
  let applyAppSettingsDefaults: (settings: AppSettings) => void;
  let setNotificationsEnabled: (enabled: boolean) => void;
  let matchMediaMock: ReturnType<typeof vi.fn>;

  function createParams(overrides: Partial<UseAppSettingsParams> = {}): UseAppSettingsParams {
    return {
      sortKey,
      sortDirection,
      compactView,
      visibleColumns,
      applyAppSettingsDefaults,
      setNotificationsEnabled,
      ...overrides,
    };
  }

  beforeEach(() => {
    resetTauriMocks();
    mockInvoke.mockImplementation(createMockInvoke());

    // Suppress expected console.error from loadSettings failures in DOM-focused tests
    vi.spyOn(console, "error").mockImplementation(() => {});

    // Reset DOM dataset
    delete document.documentElement.dataset.colorMode;
    delete document.documentElement.dataset.colorModePreference;
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.surface;

    // Mock matchMedia �?default: no dark mode preference
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

    sortKey = ref<SortKey>("added_at");
    sortDirection = ref<SortDirection>("desc");
    compactView = ref(false);
    visibleColumns = ref<string[]>([...DEFAULT_VISIBLE_COLUMNS]);
    applyAppSettingsDefaults = vi.fn();
    setNotificationsEnabled = vi.fn();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  // ── applyColorMode (via applyAppearanceSettings) ─────────────────

  describe("applyColorMode", () => {
    it('sets data-color-mode="dark" for dark mode', () => {
      const { applyAppearanceSettings } = useAppSettings(createParams());
      applyAppearanceSettings(settingsWithAppearance({ colorMode: "dark" }));

      expect(document.documentElement.dataset.colorModePreference).toBe("dark");
      expect(document.documentElement.dataset.colorMode).toBe("dark");
    });

    it('sets data-color-mode="light" for light mode', () => {
      const { applyAppearanceSettings } = useAppSettings(createParams());
      applyAppearanceSettings(settingsWithAppearance({ colorMode: "light" }));

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

      const { applyAppearanceSettings } = useAppSettings(createParams());
      applyAppearanceSettings(settingsWithAppearance({ colorMode: "system" }));

      expect(document.documentElement.dataset.colorModePreference).toBe("system");
      expect(document.documentElement.dataset.colorMode).toBe("dark");
    });
  });

  // ── applyAppearanceSettings ─────────────────────────────────────

  describe("applyAppearanceSettings", () => {
    it("sets data-theme and data-surface attributes", () => {
      const { applyAppearanceSettings } = useAppSettings(createParams());
      applyAppearanceSettings(
        settingsWithAppearance({
          themeColor: "sky",
          backgroundOpacity: "frosted",
        }),
      );

      expect(document.documentElement.dataset.theme).toBe("sky");
      expect(document.documentElement.dataset.surface).toBe("frosted");
    });

    it("falls back to defaults when appearance fields are missing", () => {
      const { applyAppearanceSettings } = useAppSettings(createParams());
      // Provide an AppSettings with undefined appearance fields by mutating
      // a properly constructed object via Object.assign (avoids type assertions).
      const settings = createDefaultSettings();
      Object.assign(settings.appearance, {
        themeColor: undefined,
        backgroundOpacity: undefined,
        colorMode: undefined,
      });
      applyAppearanceSettings(settings);

      expect(document.documentElement.dataset.theme).toBe("lime");
      expect(document.documentElement.dataset.surface).toBe("default");
      expect(document.documentElement.dataset.colorModePreference).toBe("system");
      expect(document.documentElement.dataset.colorMode).toBe("light");
    });
  });

  // ── loadSettings ────────────────────────────────────────────────

  describe("loadSettings", () => {
    it("loads settings via getAppSettings and stores in appSettings ref", async () => {
      const settings = createDefaultSettings();
      mockGetAppSettings.mockResolvedValue(settings);

      const composable = useAppSettings(createParams());
      await nextTick();

      expect(mockGetAppSettings).toHaveBeenCalledTimes(1);
      expect(composable.appSettings.value).toEqual(settings);
    });

    it("handles API error gracefully", async () => {
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      mockGetAppSettings.mockRejectedValue(new Error("Network error"));

      const composable = useAppSettings(createParams());
      await nextTick();

      expect(composable.appSettings.value).toBeNull();
      expect(consoleSpy).toHaveBeenCalledWith("Failed to load app settings", expect.any(Error));
      consoleSpy.mockRestore();
    });

    it("calls applyAppSettingsDefaults with loaded settings", async () => {
      const settings = createDefaultSettings();
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();

      expect(applyAppSettingsDefaults).toHaveBeenCalledTimes(1);
      expect(applyAppSettingsDefaults).toHaveBeenCalledWith(settings);
    });

    it("applies color mode from loaded settings", async () => {
      const settings = settingsWithAppearance({ colorMode: "dark" });
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
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

    /** Shared helper: creates composable with appSettings loaded by directly
     *  setting the ref, bypassing the async loadSettings (which doesn't play
     *  well with fake timers and debounce). */
    function createLoaded() {
      mockGetAppSettings.mockReturnValue(new Promise<AppSettings>(() => {}));
      const cmp = useAppSettings(createParams());
      cmp.appSettings.value = createDefaultSettings();
      return cmp;
    }

    it("triggers debounced saveAppSettings when sortKey changes", async () => {
      createLoaded();
      // Flush Vue queue so the {immediate: true} watcher fires and settles
      await nextTick();
      await nextTick();

      sortKey.value = "name";
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

      sortKey.value = "name";
      await nextTick();
      sortKey.value = "size";
      await nextTick();
      sortKey.value = "added_at";
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
      // Promise that never resolves so appSettings stays null
      mockGetAppSettings.mockReturnValue(new Promise<AppSettings>(() => {}));

      useAppSettings(createParams());
      await nextTick();

      sortKey.value = "name";
      await nextTick();
      await vi.advanceTimersByTimeAsync(300);

      expect(mockSaveAppSettings).not.toHaveBeenCalled();
    });

    it("saves updated compactView value", async () => {
      createLoaded();
      await nextTick();
      await nextTick();

      compactView.value = true;
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

      useAppSettings(createParams());
      await nextTick();
      await nextTick();

      expect(visibleColumns.value).toEqual(cols);
    });

    it("filters out invalid column keys", async () => {
      const settings = settingsWithAppearance({
        visibleColumns: ["file", "size", "invalid_key", "status"],
      });
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();
      await nextTick();

      // "invalid_key" should be removed
      expect(visibleColumns.value).toEqual(["file", "size", "status"]);
    });

    it("ensures 'file' column is always first", async () => {
      const settings = settingsWithAppearance({
        visibleColumns: ["size", "status", "file"],
      });
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();

      expect(visibleColumns.value[0]).toBe("file");
      // file was first, then size and status follow
      expect(visibleColumns.value).toContain("size");
      expect(visibleColumns.value).toContain("status");
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

      useAppSettings(createParams());
      await nextTick();
      await nextTick();

      expect(sortKey.value).toBe("name");
      expect(sortDirection.value).toBe("asc");
    });

    it("uses default sort key 'added_at' when settings value is null", async () => {
      const nullOverride: any = null;
      const settings = settingsWithAppearance({ sortKey: nullOverride });
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();

      expect(sortKey.value).toBe("added_at");
    });

    it("uses default sort direction 'desc' when settings value is null", async () => {
      const nullOverride: any = null;
      const settings = settingsWithAppearance({
        sortDirection: nullOverride,
      });
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();

      expect(sortDirection.value).toBe("desc");
    });
  });

  // ── Notifications sync ─────────────────────────────────────────

  describe("notifications sync", () => {
    it("calls setNotificationsEnabled with loaded settings value", async () => {
      const settings = createDefaultSettings();
      settings.notifications.enabled = true;
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();
      await nextTick();

      expect(setNotificationsEnabled).toHaveBeenCalledWith(true);
    });

    it("calls setNotificationsEnabled with false when settings has disabled notifications", async () => {
      const settings = createDefaultSettings();
      settings.notifications.enabled = false;
      mockGetAppSettings.mockResolvedValue(settings);

      useAppSettings(createParams());
      await nextTick();

      expect(setNotificationsEnabled).toHaveBeenCalledWith(false);
    });
  });
});
