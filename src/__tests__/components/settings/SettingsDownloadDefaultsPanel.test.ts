import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import SettingsDownloadDefaultsPanel from "../../../components/settings/SettingsDownloadDefaultsPanel.vue";
import type { AppSettings } from "../../../types/settings";
import type { ChecksumMode } from "../../../types/download";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  SettingsSection: {
    template:
      '<div class="settings-section-stub">{{ title }}{{ summary ? "|" + summary : "" }}<slot /></div>',
    props: ["title", "icon", "summary"],
  },
  SettingsField: {
    template: '<div class="settings-field-stub"><slot /></div>',
    props: ["title", "icon", "wide", "infoTooltip", "labelFor"],
  },
  UiTextField: {
    template:
      '<input class="ui-text-field-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" :placeholder="placeholder" :type="type" :min="min" :max="max" />',
    props: ["modelValue", "placeholder", "type", "min", "max"],
  },
  UiButton: {
    template:
      '<button class="ui-button-stub" :disabled="loading" @click="$emit(\'click\')"><slot /></button>',
    props: ["loading"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option></select>',
    props: ["modelValue", "options"],
  },
  InfoTooltip: {
    template: '<span class="info-tooltip-stub" />',
    props: ["text"],
  },
  Teleport: false,
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
      defaultDownloadDir: "/downloads",
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

const checksumOptions: Array<{ label: string; value: ChecksumMode }> = [
  { label: "None", value: "none" },
  { label: "BLAKE3", value: "blake3" },
  { label: "SHA-256", value: "sha256" },
  { label: "XXH3_128", value: "xxh3_128" },
];

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsDownloadDefaultsPanel", () => {
  it("renders directory field with v-model and browse button", () => {
    const draft = reactive(createSettings());
    draft.download.defaultDownloadDir = "/my/downloads";

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    // Directory text field has the correct v-model value
    const textFields = wrapper.findAll<HTMLInputElement>(".ui-text-field-stub");
    const dirField = textFields[0];
    expect(dirField.element.value).toBe("/my/downloads");

    // Browse button is rendered with correct label
    const buttons = wrapper.findAll(".ui-button-stub");
    const browseBtn = buttons[0];
    expect(browseBtn.text()).toBe("common.browse");

    // Change directory value
    void dirField.setValue("/new/path");
    expect(draft.download.defaultDownloadDir).toBe("/new/path");
  });

  it("emits pickDirectory when browse button clicked", () => {
    const draft = reactive(createSettings());

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const buttons = wrapper.findAll(".ui-button-stub");
    const browseBtn = buttons[0];
    void browseBtn.trigger("click");

    expect(wrapper.emitted("pickDirectory")).toBeTruthy();
  });

  it('shows "Browsing…" on button when isPickingDirectory is true', () => {
    const draft = reactive(createSettings());

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: true,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const buttons = wrapper.findAll(".ui-button-stub");
    const browseBtn = buttons[0];
    expect(browseBtn.text()).toBe("common.browsing");
  });

  it("shows loading state on button when isPickingDirectory is true", () => {
    const draft = reactive(createSettings());

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: true,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const buttons = wrapper.findAll(".ui-button-stub");
    const browseBtn = buttons[0];
    expect(browseBtn.attributes("disabled")).toBeDefined();
  });

  it("renders retries number field with correct min/max", () => {
    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft: reactive(createSettings()),
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const textFields = wrapper.findAll<HTMLInputElement>(".ui-text-field-stub");
    // Second text field is the retries field
    const retriesField = textFields[1];
    expect(retriesField.attributes("type")).toBe("number");
    expect(retriesField.attributes("min")).toBe("0");
    expect(retriesField.attributes("max")).toBe("20");
  });

  it("v-model on retries updates draft.download.defaultMaxRetries", () => {
    const draft = reactive(createSettings());
    draft.download.defaultMaxRetries = 3;

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const textFields = wrapper.findAll<HTMLInputElement>(".ui-text-field-stub");
    const retriesField = textFields[1];
    expect(retriesField.element.value).toBe("3");

    void retriesField.setValue("10");
    expect(draft.download.defaultMaxRetries).toBe("10");
  });

  it("renders checksum select with options", () => {
    const draft = reactive(createSettings());
    draft.download.defaultChecksum = "sha256";

    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft,
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Default browser UA",
      },
      global: { stubs },
    });

    const select = wrapper.find<HTMLSelectElement>(".ui-select-stub");
    expect(select.exists()).toBe(true);
    expect(select.element.value).toBe("sha256");

    // Verify all options are rendered
    const options = select.findAll("option");
    expect(options).toHaveLength(4);
    expect(options[0].text()).toBe("None");
    expect(options[0].attributes("value")).toBe("none");
    expect(options[1].text()).toBe("BLAKE3");
    expect(options[1].attributes("value")).toBe("blake3");

    // Change selection
    void select.setValue("none");
    expect(draft.download.defaultChecksum).toBe("none");
  });

  it("renders user agent field with placeholder", () => {
    const wrapper = mount(SettingsDownloadDefaultsPanel, {
      props: {
        draft: reactive(createSettings()),
        t: (key: string) => key,
        checksumOptions,
        downloadSummary: "5 retries, blake3",
        isPickingDirectory: false,
        defaultUserAgentPlaceholder: "Mozilla/5.0 (compatible; Limedl/1.0)",
      },
      global: { stubs },
    });

    const textFields = wrapper.findAll<HTMLInputElement>(".ui-text-field-stub");
    // Third text field is the user agent field
    const uaField = textFields[2];
    expect(uaField.exists()).toBe(true);
    expect(uaField.attributes("placeholder")).toBe("Mozilla/5.0 (compatible; Limedl/1.0)");
  });
});
