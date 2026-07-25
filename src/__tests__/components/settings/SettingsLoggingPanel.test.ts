import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import SettingsLoggingPanel from "../../../components/settings/SettingsLoggingPanel.vue";
import type { AppSettings, LogLevel } from "../../../types/settings";

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
    template:
      '<div class="settings-field-stub"><slot /></div>',
    props: ["title", "icon", "wide", "infoTooltip", "labelFor"],
  },
  UiSwitch: {
    template:
      '<input type="checkbox" class="ui-switch-stub" :checked="modelValue" @change="$emit(\'update:modelValue\', $event.target.checked)" />',
    props: ["modelValue"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option></select>',
    props: ["modelValue", "options"],
  },
  UiTextField: {
    template:
      '<input class="ui-text-field-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" :placeholder="placeholder" :type="type" :min="min" :max="max" />',
    props: ["modelValue", "placeholder", "type", "min", "max"],
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
      filePath: "/var/log/limedl.log",
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
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    speedLimitSchedule: [],
  };
}

const logLevelOptions: Array<{ label: string; value: LogLevel }> = [
  { label: "Trace", value: "trace" },
  { label: "Debug", value: "debug" },
  { label: "Info", value: "info" },
  { label: "Warn", value: "warn" },
  { label: "Error", value: "error" },
];

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsLoggingPanel", () => {
  it("renders enabled switch with correct v-model binding to draft.logging.enabled", () => {
    const draft = reactive(createSettings());
    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
      },
      global: { stubs },
    });

    const switchInput = wrapper.find<HTMLInputElement>(".ui-switch-stub");
    expect(switchInput.exists()).toBe(true);
    expect(switchInput.element.checked).toBe(true);

    // Toggle off
    void switchInput.setValue(false);
    expect(draft.logging.enabled).toBe(false);

    // Toggle back on
    void switchInput.setValue(true);
    expect(draft.logging.enabled).toBe(true);
  });

  it("renders level select with correct v-model and options", () => {
    const draft = reactive(createSettings());
    draft.logging.level = "debug";

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
      },
      global: { stubs },
    });

    const select = wrapper.find<HTMLSelectElement>(".ui-select-stub");
    expect(select.exists()).toBe(true);
    expect(select.element.value).toBe("debug");

    // Verify options are rendered
    const options = select.findAll("option");
    expect(options).toHaveLength(5);
    expect(options[0].text()).toBe("Trace");
    expect(options[0].attributes("value")).toBe("trace");
    expect(options[2].text()).toBe("Info");
    expect(options[2].attributes("value")).toBe("info");

    // Change selection
    void select.setValue("warn");
    expect(draft.logging.level).toBe("warn");
  });

  it("renders file path text field with v-model and placeholder", () => {
    const draft = reactive(createSettings());
    draft.logging.filePath = "/custom/path/limedl.log";

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
      },
      global: { stubs },
    });

    const textField = wrapper.find<HTMLInputElement>(".ui-text-field-stub");
    expect(textField.exists()).toBe(true);
    expect(textField.element.value).toBe("/custom/path/limedl.log");

    // Verify placeholder is the i18n key
    expect(textField.attributes("placeholder")).toBe("settings.loggingPathPlaceholder");

    // Change value
    void textField.setValue("/new/path/log.txt");
    expect(draft.logging.filePath).toBe("/new/path/log.txt");
  });

  it("renders all three fields within SettingsSection", () => {
    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft: reactive(createSettings()),
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
      },
      global: { stubs },
    });

    // SettingsSection is rendered with title
    expect(wrapper.find(".settings-section-stub").exists()).toBe(true);

    // All three field stubs are rendered
    expect(wrapper.find(".ui-switch-stub").exists()).toBe(true);
    expect(wrapper.find(".ui-select-stub").exists()).toBe(true);
    expect(wrapper.find(".ui-text-field-stub").exists()).toBe(true);
  });

  it("passes loggingSummary to SettingsSection", () => {
    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft: reactive(createSettings()),
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "3 logs retained",
      },
      global: { stubs },
    });

    const section = wrapper.find(".settings-section-stub");
    expect(section.text()).toContain("3 logs retained");
  });
});
