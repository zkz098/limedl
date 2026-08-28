import { describe, it, expect, vi, beforeAll } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import SettingsProxyPanel from "../../../components/settings/SettingsProxyPanel.vue";
import type { AppSettings, ProxyMode } from "../../../types/settings";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("@vueuse/core", () => ({
  onClickOutside: vi.fn(() => () => {}),
}));

// jsdom does not implement scrollIntoView
beforeAll(() => {
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
});

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  Teleport: { template: "<div><slot /></div>" },
  SettingsSection: {
    template:
      '<section class="settings-section-stub"><h3 v-if="title">{{ title }}</h3><slot /><span v-if="summary" class="summary-stub">{{ summary }}</span></section>',
    props: ["title", "icon", "summary"],
  },
  SettingsField: {
    template:
      '<div class="settings-field-stub"><span v-if="label" class="settings-field__label-stub">{{ label }}</span><slot /><p v-if="hint" class="settings-field__hint-stub">{{ hint }}</p></div>',
    props: ["label", "hint", "infoTooltip", "wide"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="opt in options" :key="opt.value" :value="opt.value">{{ opt.label }}</option></select>',
    props: ["modelValue", "options"],
  },
  UiTextField: {
    template:
      '<span class="ui-textfield-stub"><input class="ui-textfield__input-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" /></span>',
    props: ["modelValue", "placeholder", "type"],
  },
  InfoTooltip: {
    template: '<span class="info-tooltip-stub" />',
    props: ["text"],
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
      autoDetectSha256: true,
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
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    doubleClick: { onCompleted: "none", onUncompleted: "none" },
    speedLimitSchedule: [],
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsProxyPanel", () => {
  it("renders mode select and section header", () => {
    const draft = reactive(createSettings());
    const proxyModeOptions: Array<{ label: string; value: ProxyMode }> = [
      { label: "Disabled", value: "disabled" },
      { label: "Manual", value: "manual" },
      { label: "System", value: "system" },
    ];

    const wrapper = mount(SettingsProxyPanel, {
      props: {
        draft,
        t: (key: string) => key,
        proxyModeOptions,
        proxySummary: "Proxy disabled",
      },
      global: { stubs },
    });

    // Section header renders
    expect(wrapper.text()).toContain("settings.proxyTitle");

    // Mode select exists
    const select = wrapper.find(".ui-select-stub");
    expect(select.exists()).toBe(true);

    // Mode label exists
    expect(wrapper.text()).toContain("settings.proxyMode");

    // Summary text is rendered
    expect(wrapper.text()).toContain("Proxy disabled");
  });
});
