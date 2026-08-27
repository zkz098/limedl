import { describe, it, expect, vi, beforeEach } from "vitest";
import { nextTick, reactive, ref, type Ref } from "vue";

import { useSetupWizardLifecycle } from "../../composables/useSetupWizardLifecycle";
import type { AppSettings } from "../../types/settings";

// ── Mocks ──────────────────────────────────────────────────────────

// The lifecycle touches settings-api and app-api; these tests only
// exercise the setup-state path, so plain mocks suffice.
vi.mock("../../lib/tauri/settings-api", () => ({
  getAppSettings: vi.fn(),
  saveAppSettings: vi.fn(),
}));

vi.mock("../../lib/tauri/app-api", () => ({
  getAppInfo: vi.fn().mockResolvedValue({
    name: "limedl",
    version: "0.1.8",
    platform: "linux",
    arch: "x86_64",
  }),
}));

// ── Fixtures ───────────────────────────────────────────────────────

function createDefaultSettings(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "lime",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: true,
      showHeatmap: true,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: ["file", "size", "status"],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "traditional",
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 16,
        maxThreadsPerTask: 8,
        minThreadsPerTask: 0,
        adaptiveProfile: "balanced",
      },
      chunkSizeStrategy: "adaptive",
      tailSprintEnabled: false,
      connectionWarmupEnabled: false,
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 5,
      defaultChecksum: "blake3",
      defaultUserAgent: "Mozilla/5.0",
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
      upnpEnabled: false,
      enableNatpmp: true,
      enableIpv6: true,
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
      maxSeeds: 5,
      maxTorrents: 100,
      activeLimit: 500,
    },
    logging: {
      enabled: true,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: true, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    urlRewrite: { enabled: false, rules: [] },
    notifications: { enabled: true },
    ioBaseline: {
      bufferLimitMb: 1024,
      gameModeBufferMb: 128,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 4,
      gameModeMaxParallel: 1,
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

// ── Helpers ────────────────────────────────────────────────────────

function makeLifecycle(appSettings: Ref<AppSettings | null>) {
  return useSetupWizardLifecycle({
    appSettings,
    applyAppearanceSettings: vi.fn(),
    applyAppSettingsDefaults: vi.fn(),
    setNotificationsEnabled: vi.fn(),
  });
}

// ── Tests ──────────────────────────────────────────────────────────

describe("useSetupWizardLifecycle", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  it("shows the wizard when setup is incomplete", async () => {
    const appSettings = ref<AppSettings | null>(null);
    const lifecycle = makeLifecycle(appSettings);

    expect(lifecycle.showSetupWizard.value).toBeNull();

    // Simulate the store finishing init (App.vue onMounted → initStore).
    appSettings.value = reactive(createDefaultSettings({ setupCompleted: false }));
    await nextTick();

    expect(lifecycle.showSetupWizard.value).toBe(true);
    expect(lifecycle.setupInitialSettings.value).toBeDefined();
  });

  it("passes a plain deep copy of appSettings to the wizard, not the reactive proxy", async () => {
    // Regression: appSettings flows from a pinia store (reactive proxy). Passing
    // the proxy through to the wizard made structuredClone throw DataCloneError,
    // aborting the wizard's setup function and blanking the UI.
    const storeSettings = reactive(createDefaultSettings({ setupCompleted: false }));
    const appSettings = ref<AppSettings | null>(null);
    const lifecycle = makeLifecycle(appSettings);

    appSettings.value = storeSettings;
    await nextTick();

    const initial = lifecycle.setupInitialSettings.value;
    expect(initial).not.toBeNull();
    expect(lifecycle.showSetupWizard.value).toBe(true);

    // Same data, but not the same (proxy) reference.
    expect(initial).toEqual(storeSettings);
    expect(initial).not.toBe(storeSettings);

    // Root and nested levels must be plain objects, not proxies.
    expect(Object.getPrototypeOf(initial)).toBe(Object.prototype);
    expect(Object.getPrototypeOf(initial!.appearance)).toBe(Object.prototype);
    expect(Object.getPrototypeOf(initial!.bt)).toBe(Object.prototype);

    // Deep copy: mutating the wizard snapshot must not leak into the store.
    initial!.appearance.themeColor = "amber";
    initial!.bt.dhtEnabled = false;
    expect(storeSettings.appearance.themeColor).toBe("lime");
    expect(storeSettings.bt.dhtEnabled).toBe(true);
  });

  it("skips the wizard when setup was already completed", async () => {
    const appSettings = ref<AppSettings | null>(null);
    const lifecycle = makeLifecycle(appSettings);

    appSettings.value = reactive(createDefaultSettings({ setupCompleted: true }));
    await nextTick();

    expect(lifecycle.showSetupWizard.value).toBe(false);
    expect(lifecycle.setupInitialSettings.value).toBeNull();
    expect(localStorage.getItem("limedl.setupCompleted")).toBe("true");
  });
});
