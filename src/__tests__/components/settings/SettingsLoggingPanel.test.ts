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
    template: '<div class="settings-field-stub"><slot /></div>',
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

const logLevelOptions: Array<{ label: string; value: LogLevel }> = [
  { label: "Trace", value: "trace" },
  { label: "Debug", value: "debug" },
  { label: "Info", value: "info" },
  { label: "Warn", value: "warn" },
  { label: "Error", value: "error" },
];

function findStrategySelect(wrapper: ReturnType<typeof mount>) {
  return wrapper
    .findAll(".ui-select-stub")
    .find((select) => select.find('option[value="none"]').exists());
}

function findCountInput(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll(".ui-text-field-stub").find((input) => input.attributes("max") === "1000");
}

function findDaysInput(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll(".ui-text-field-stub").find((input) => input.attributes("max") === "3650");
}

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
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
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
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
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
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
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

  it("renders logging panel fields within SettingsSection", () => {
    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft: reactive(createSettings()),
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    // SettingsSection is rendered with title
    expect(wrapper.find(".settings-section-stub").exists()).toBe(true);

    // All field stubs are rendered
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
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    const section = wrapper.find(".settings-section-stub");
    expect(section.text()).toContain("3 logs retained");
  });

  it("renders retention strategy select and changing strategy updates model", async () => {
    const draft = reactive(createSettings());
    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    const strategySelect = findStrategySelect(wrapper);
    expect(strategySelect).toBeDefined();
    expect(strategySelect!.exists()).toBe(true);

    const options = strategySelect!.findAll("option");
    expect(options).toHaveLength(4);
    expect(options.map((option) => option.attributes("value"))).toEqual([
      "none",
      "count",
      "days",
      "both",
    ]);

    await strategySelect!.setValue("count");
    expect(draft.logging.retentionDays).toBeNull();
    expect(draft.logging.retentionCount).not.toBeNull();
  });

  it("shows count input when strategy is count and hides it for days", async () => {
    const draft = reactive(createSettings());
    draft.logging.retentionCount = 10;
    draft.logging.retentionDays = null;

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    expect(findCountInput(wrapper)).toBeDefined();
    expect(findDaysInput(wrapper)).toBeUndefined();

    const strategySelect = findStrategySelect(wrapper);
    await strategySelect!.setValue("days");

    expect(findCountInput(wrapper)).toBeUndefined();
    expect(findDaysInput(wrapper)).toBeDefined();
  });

  it("shows days input when strategy is days and hides it for count", async () => {
    const draft = reactive(createSettings());
    draft.logging.retentionCount = null;
    draft.logging.retentionDays = 30;

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    expect(findDaysInput(wrapper)).toBeDefined();
    expect(findCountInput(wrapper)).toBeUndefined();

    const strategySelect = findStrategySelect(wrapper);
    await strategySelect!.setValue("count");

    expect(findDaysInput(wrapper)).toBeUndefined();
    expect(findCountInput(wrapper)).toBeDefined();
  });

  it("setting strategy to count clears days and vice versa", async () => {
    const draft = reactive(createSettings());
    draft.logging.retentionCount = null;
    draft.logging.retentionDays = 30;

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    const strategySelect = findStrategySelect(wrapper);

    await strategySelect!.setValue("count");
    expect(draft.logging.retentionDays).toBeNull();
    expect(draft.logging.retentionCount).toBe(10);

    await strategySelect!.setValue("days");
    expect(draft.logging.retentionCount).toBeNull();
    expect(draft.logging.retentionDays).toBe(30);
  });

  it("setting strategy to none clears both retention fields", async () => {
    const draft = reactive(createSettings());
    draft.logging.retentionCount = 10;
    draft.logging.retentionDays = 30;

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    const strategySelect = findStrategySelect(wrapper);
    await strategySelect!.setValue("none");

    expect(draft.logging.retentionCount).toBeNull();
    expect(draft.logging.retentionDays).toBeNull();
  });

  it("shows both retention inputs when strategy is both", async () => {
    const draft = reactive(createSettings());
    draft.logging.retentionCount = 10;
    draft.logging.retentionDays = 30;

    const wrapper = mount(SettingsLoggingPanel, {
      props: {
        draft,
        t: (key: string) => key,
        logLevelOptions,
        loggingSummary: "Logging enabled",
        isPickingLogDirectory: false,
        isOpeningLogDir: false,
      },
      global: { stubs },
    });

    expect(findCountInput(wrapper)).toBeDefined();
    expect(findDaysInput(wrapper)).toBeDefined();

    const strategySelect = findStrategySelect(wrapper);
    await strategySelect!.setValue("none");

    expect(findCountInput(wrapper)).toBeUndefined();
    expect(findDaysInput(wrapper)).toBeUndefined();
  });
});
