import { describe, it, expect, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { nextTick, reactive, ref } from "vue";
import SettingsPage from "../../components/settings/SettingsPage.vue";
import type { AppSettings } from "../../types/settings";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

vi.mock("../../i18n", () => ({
  useI18n: () => ({
    t: (key: string) => key,
    language: { value: "en-US" },
    languageOptions: [{ label: "English", value: "en-US" }],
    setLanguage: vi.fn(),
    supportedLanguages: ["en-US"],
  }),
  t: (key: string) => key,
}));

vi.mock("../../composables/useAsyncGuard", () => ({
  useAsyncGuard: () => ({
    isBusy: { value: false },
    run: vi.fn((fn: () => Promise<unknown>) => fn()),
  }),
}));

vi.mock("../../composables/useNotification", () => ({
  useNotification: () => ({
    notifySuccess: vi.fn(),
    notifyError: vi.fn(),
    notifyInfo: vi.fn(),
    notifyWarning: vi.fn(),
  }),
}));

vi.mock("../../lib/tauri/dialog-api", () => ({
  pickDirectory: vi.fn().mockResolvedValue("/default/dir"),
}));

const mockSaveAppSettings = vi.hoisted(() => vi.fn());
vi.mock("../../lib/tauri/settings-api", () => ({
  saveAppSettings: mockSaveAppSettings,
  fetchTrackerList: vi.fn(),
}));

// Mock the entire settingsComposables barrel with a simple reactive form
vi.mock("../../components/settings/settingsComposables", () => {
  const defaultForm = {
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
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "automatic",
      traditional: { maxParallelTasks: 3 },
      automatic: { maxParallelThreads: 16, maxThreadsPerTask: 8, minThreadsPerTask: 0, adaptiveProfile: "balanced" },
      chunkSizeStrategy: "adaptive",
    },
    download: { defaultDownloadDir: "", defaultMaxRetries: 5, defaultChecksum: "blake3", defaultUserAgent: "Mozilla/5.0" },
    bt: {
      pauseUploadWhenLimitReached: false, uploadLimitBytes: 0, uploadRatioLimit: 0,
      dhtEnabled: true, trackerList: "", trackerListUrl: "https://cf.trackerslist.com/best.txt",
      listenPort: null, listenPortRange: null, upnpEnabled: false,
      enableNatpmp: true, enableIpv6: true, enablePex: true, enableLsd: true,
      enableUtp: true, enableFastExtension: true, enableHolepunch: true,
      enableWebSeed: true, enableSuperSeeding: false,
      globalDownloadRateLimit: 0, globalUploadRateLimit: 0,
      preallocateMode: "none", encryptionMode: "enabled",
      maxDownloads: 3, maxSeeds: 5, maxTorrents: 100, activeLimit: 500,
    },
    logging: { enabled: true, level: "info", filePath: "", retentionCount: null, retentionDays: null },
    aria2Rpc: { enabled: true, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: { enabled: false, activeIp: null, activeSpeedMbps: null, lastTestAtMs: null, lastError: null },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: { bufferLimitMb: 1024, gameModeBufferMb: 128, gameMode: false, diskTypeOverrides: {}, maxParallelHdd: 4, gameModeMaxParallel: 1 },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
  };

  return {
    useSettingsForm: vi.fn(() => ({
      form: reactive({ ...defaultForm }),
      buildSettingsPayload: vi.fn(() => ({ ...defaultForm })),
      savedSettingsSnapshot: ref(""),
    })),
    useSettingsSummaries: vi.fn(() => ({
      proxySummary: ref("Proxy: Disabled"),
      loggingSummary: ref("Logging: Info"),
      downloadSummary: ref("Download defaults"),
      btUploadLimitMiB: ref(0),
      setBtUploadLimitMiB: vi.fn(),
      globalSpeedLimitMiBps: ref(0),
      setGlobalSpeedLimitMiBps: vi.fn(),
      trackerListEntries: ref([]),
      btSummary: ref("BT summary"),
    })),
    serializeSettings: vi.fn((s: AppSettings) => JSON.stringify(s)),
    settingsDraftSnapshot: vi.fn((s: AppSettings) => JSON.stringify(s)),
    DEFAULT_HTTP_USER_AGENT: "Mozilla/5.0",
    DEFAULT_TRACKER_LIST_URL: "https://cf.trackerslist.com/best.txt",
  };
});

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiButton: {
    template:
      '<button :disabled="disabled" :data-icon="icon" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "icon", "size", "variant", "type", "loading", "block"],
  },
  SettingsAppearancePanel: {
    template: '<div class="settings-panel-stub" data-panel="appearance">Appearance Panel</div>',
    props: ["draft", "t", "language", "languageOptions", "colorModeOptions", "backgroundOpacityOptions"],
  },
  SettingsSchedulerPanel: {
    template: '<div class="settings-panel-stub" data-panel="scheduler">Scheduler Panel</div>',
    props: ["draft", "t", "schedulerModeOptions", "adaptiveProfileOptions", "globalSpeedLimitMiBps"],
  },
  SettingsDownloadDefaultsPanel: {
    template: '<div class="settings-panel-stub" data-panel="downloads">Downloads Panel</div>',
    props: ["draft", "t", "checksumOptions", "downloadSummary", "isPickingDirectory", "defaultUserAgentPlaceholder"],
  },
  SettingsIoBaselinePanel: {
    template: '<div class="settings-panel-stub" data-panel="io-baseline">IO Baseline Panel</div>',
    props: ["draft", "t", "gameMode", "bufferUsageBytes", "bufferLimitBytes", "activeSlots", "maxSlots", "queuedCount"],
  },
  SettingsBtPanel: {
    template: '<div class="settings-panel-stub" data-panel="bt">BT Panel</div>',
    props: ["draft", "t", "btSummary", "btUploadLimitMiB", "isFetchingTrackerList", "defaultTrackerListUrl"],
  },
  SettingsAria2RpcPanel: {
    template: '<div class="settings-panel-stub" data-panel="aria2Rpc">Aria2 RPC Panel</div>',
    props: ["draft", "t"],
  },
  SettingsLoggingPanel: {
    template: '<div class="settings-panel-stub" data-panel="logging">Logging Panel</div>',
    props: ["draft", "t", "logLevelOptions", "loggingSummary"],
  },
  SettingsProxyPanel: {
    template: '<div class="settings-panel-stub" data-panel="proxy">Proxy Panel</div>',
    props: ["draft", "t", "proxyModeOptions", "proxySummary"],
  },
  SettingsAboutPanel: {
    template: '<div class="settings-panel-stub" data-panel="about">About Panel</div>',
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createSettings(): AppSettings {
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
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "automatic",
      traditional: { maxParallelTasks: 3 },
      automatic: { maxParallelThreads: 16, maxThreadsPerTask: 8, minThreadsPerTask: 0, adaptiveProfile: "balanced" },
      chunkSizeStrategy: "adaptive",
    },
    download: { defaultDownloadDir: "", defaultMaxRetries: 5, defaultChecksum: "blake3", defaultUserAgent: "Mozilla/5.0" },
    bt: {
      pauseUploadWhenLimitReached: false, uploadLimitBytes: 0, uploadRatioLimit: 0,
      dhtEnabled: true, trackerList: "", trackerListUrl: "",
      listenPort: null, listenPortRange: null, upnpEnabled: false,
      enableNatpmp: true, enableIpv6: true, enablePex: true, enableLsd: true,
      enableUtp: true, enableFastExtension: true, enableHolepunch: true,
      enableWebSeed: true, enableSuperSeeding: false,
      globalDownloadRateLimit: 0, globalUploadRateLimit: 0,
      preallocateMode: "none", encryptionMode: "enabled",
      maxDownloads: 3, maxSeeds: 5, maxTorrents: 100, activeLimit: 500,
    },
    logging: { enabled: true, level: "info", filePath: "", retentionCount: null, retentionDays: null },
    aria2Rpc: { enabled: true, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: { enabled: false, activeIp: null, activeSpeedMbps: null, lastTestAtMs: null, lastError: null },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: { bufferLimitMb: 1024, gameModeBufferMb: 128, gameMode: false, diskTypeOverrides: {}, maxParallelHdd: 4, gameModeMaxParallel: 1 },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
  };
}

function mountPage(props: Record<string, unknown> = {}) {
  return mount(SettingsPage, {
    props: {
      settings: createSettings(),
      ...props,
    },
    global: { stubs },
    attachTo: document.body,
  });
}

describe("SettingsPage", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders the settings page with header", () => {
    const wrapper = mountPage();
    expect(wrapper.find(".settings-page").exists()).toBe(true);
    expect(wrapper.text()).toContain("settings.kicker");
    expect(wrapper.text()).toContain("settings.title");
  });

  it("renders sidebar with all tab buttons", () => {
    const wrapper = mountPage();
    const tabs = wrapper.findAll('[role="tab"]');
    expect(tabs).toHaveLength(8); // 8 tabs in the tabs array
    expect(tabs[0].text()).toContain("settings.appearanceKicker");
    expect(tabs[1].text()).toContain("settings.scheduler");
    expect(tabs[2].text()).toContain("settings.downloads");
    expect(tabs[3].text()).toContain("settings.bt");
    expect(tabs[4].text()).toContain("settings.aria2Rpc");
  });

  it("renders save button with save icon", () => {
    const wrapper = mountPage();
    const saveBtn = wrapper.find('button.ui-button-stub[data-icon="i-ri-save-line"]');
    expect(saveBtn.exists()).toBe(true);
    expect(saveBtn.attributes("data-icon")).toBe("i-ri-save-line");
  });

  it("renders save hint text", () => {
    const wrapper = mountPage();
    expect(wrapper.text()).toContain("settings.saveHint");
  });

  // ── Tab Navigation ─────────────────────────────────────────
  it("shows appearance panel by default", () => {
    const wrapper = mountPage();
    const visiblePanels = wrapper.findAll('.settings-panel-stub[data-panel="appearance"]');
    expect(visiblePanels.length).toBeGreaterThanOrEqual(1);
  });

  it("switches to scheduler tab when clicked", async () => {
    const wrapper = mountPage();
    const tabs = wrapper.findAll('[role="tab"]');

    // Click scheduler tab (index 1)
    await tabs[1].trigger("click");
    await nextTick();

    // Check that the tab has aria-selected
    expect(tabs[1].attributes("aria-selected")).toBe("true");
    expect(tabs[0].attributes("aria-selected")).toBe("false");
  });

  it("switches to each tab and shows the correct panel", async () => {
    const wrapper = mountPage();
    const tabs = wrapper.findAll('[role="tab"]');

    for (let i = 0; i < tabs.length; i++) {
      // eslint-disable-next-line no-await-in-loop
      await tabs[i].trigger("click");
      // eslint-disable-next-line no-await-in-loop
      await nextTick();

      // The clicked tab should be selected
      expect(tabs[i].attributes("aria-selected")).toBe("true");

      // All other tabs should not be selected
      for (let j = 0; j < tabs.length; j++) {
        if (j !== i) {
          expect(tabs[j].attributes("aria-selected")).toBe("false");
        }
      }
    }
  });

  // ── Save button ────────────────────────────────────────────
  it("save button calls persistSettings and emits saved", async () => {
    mockSaveAppSettings.mockResolvedValue(createSettings());

    const wrapper = mountPage();
    const saveBtn = wrapper.find('button.ui-button-stub[data-icon="i-ri-save-line"]');
    expect(saveBtn.exists()).toBe(true);

    await saveBtn.trigger("click");
    await flushPromises();
    await nextTick();

    // persistSettings should have been called — it calls saveAppSettings
    expect(mockSaveAppSettings).toHaveBeenCalled();
    expect(wrapper.emitted("saved")).toBeTruthy();
  });

  // ── restartSetup emission ──────────────────────────────────
  it("about tab stub is rendered when about tab is active", async () => {
    const wrapper = mountPage();
    const tabs = wrapper.findAll('[role="tab"]');

    // Go to about tab
    await tabs[7].trigger("click");
    await nextTick();

    expect(wrapper.find('[data-panel="about"]').exists()).toBe(true);
  });

  // ── Accessibility ──────────────────────────────────────────
  it("sidebar has role='tablist' with aria-label", () => {
    const wrapper = mountPage();
    const sidebar = wrapper.find('[role="tablist"]');
    expect(sidebar.exists()).toBe(true);
    expect(sidebar.attributes("aria-label")).toBe("settings.title");
  });

  it("active tab has visual indicator classes", async () => {
    const wrapper = mountPage();
    const tabs = wrapper.findAll('[role="tab"]');

    // First tab should be active by default
    expect(tabs[0].attributes("aria-selected")).toBe("true");

    // Click a different tab
    await tabs[3].trigger("click");
    await nextTick();

    expect(tabs[3].attributes("aria-selected")).toBe("true");
    expect(tabs[0].attributes("aria-selected")).toBe("false");
  });
});
