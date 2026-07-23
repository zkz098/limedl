import { describe, it, expect, vi, beforeAll } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive, nextTick } from "vue";
import SettingsSchedulerPanel from "../../../components/settings/SettingsSchedulerPanel.vue";
import type { AppSettings, SchedulerMode, AdaptiveProfile } from "../../../types/settings";

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
      '<section class="settings-section-stub"><h3 v-if="title">{{ title }}</h3><slot /></section>',
    props: ["title", "icon"],
  },
  SettingsField: {
    template:
      '<div class="settings-field-stub" :class="{ \'settings-field--wide\': wide }"><span v-if="label" class="settings-field__label-stub">{{ label }}</span><slot /><p v-if="hint" class="settings-field__hint-stub">{{ hint }}</p></div>',
    props: ["label", "hint", "infoTooltip", "wide"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="opt in options" :key="opt.value" :value="opt.value">{{ opt.label }}</option></select>',
    props: ["modelValue", "options"],
  },
  UiTextField: {
    template:
      '<span class="ui-textfield-stub" :data-unit="unit || null"><input class="ui-textfield__input-stub" :value="modelValue" @input="$emit(\'update:modelValue\', Number($event.target.value))" /><span v-if="unit" class="ui-textfield__unit-stub">{{ unit }}</span></span>',
    props: ["modelValue", "min", "max", "disabled", "type", "unit"],
  },
  UiSwitch: {
    template:
      '<input type="checkbox" class="ui-switch-stub" :checked="modelValue" @change="$emit(\'update:modelValue\', $event.target.checked)" />',
    props: ["modelValue", "label", "disabled"],
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
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
  };
}

// ── Helpers ────────────────────────────────────────────────────────

function mountPanel(props: Record<string, unknown> = {}) {
  const draft = reactive(createSettings());
  return {
    draft,
    wrapper: mount(SettingsSchedulerPanel, {
      props: {
        draft,
        t: (key: string) => key,
        schedulerModeOptions: [
          { label: "Automatic", value: "automatic" as SchedulerMode },
          { label: "Traditional", value: "traditional" as SchedulerMode },
        ],
        adaptiveProfileOptions: [
          { label: "Conservative", value: "conservative" as AdaptiveProfile },
          { label: "Balanced", value: "balanced" as AdaptiveProfile },
          { label: "Aggressive", value: "aggressive" as AdaptiveProfile },
        ],
        globalSpeedLimitMiBps: 0,
        ...props,
      },
      global: { stubs },
    }),
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsSchedulerPanel", () => {
  // ── Test 1: Default rendering (automatic mode) ──────────────
  it("renders with automatic mode fields visible by default", () => {
    const { wrapper } = mountPanel();

    // Mode select is visible
    expect(wrapper.find(".ui-select-stub").exists()).toBe(true);

    // Allocation mode label is rendered
    expect(wrapper.text()).toContain("settings.allocationMode");

    // Automatic mode fields are visible
    expect(wrapper.text()).toContain("settings.maxParallelThreads");
    expect(wrapper.text()).toContain("settings.maxThreadsPerTask");
    expect(wrapper.text()).toContain("settings.minThreadsPerTask");
    expect(wrapper.text()).toContain("settings.adaptiveProfile");

    // Traditional mode field is NOT visible
    expect(wrapper.text()).not.toContain("settings.maxParallelTasks");
    expect(wrapper.text()).not.toContain("settings.traditionalHint");
  });

  // ── Test 2: Switch to traditional mode ──────────────────────
  it("switches to traditional mode and shows traditional fields", async () => {
    const { draft, wrapper } = mountPanel();

    // Switch draft to traditional mode
    draft.scheduler.mode = "traditional";
    await nextTick();

    // Traditional mode fields become visible
    expect(wrapper.text()).toContain("settings.maxParallelTasks");
    expect(wrapper.text()).toContain("settings.traditionalHint");

    // Automatic mode fields become hidden
    expect(wrapper.text()).not.toContain("settings.maxParallelThreads");
    expect(wrapper.text()).not.toContain("settings.maxThreadsPerTask");
    expect(wrapper.text()).not.toContain("settings.minThreadsPerTask");
    expect(wrapper.text()).not.toContain("settings.adaptiveProfile");
  });

  // ── Test 3: Global speed limit field ────────────────────────
  it("renders global speed limit field with label and unit", () => {
    const { wrapper } = mountPanel({ globalSpeedLimitMiBps: 100 });

    // Label is rendered (hint is an info-tooltip, not visible as text)
    expect(wrapper.text()).toContain("settings.globalSpeedLimit");

    // Unit suffix is rendered on the speed limit field
    const unitSpans = wrapper.findAll(".ui-textfield__unit-stub");
    const miBsUnit = unitSpans.find((el) => el.text() === "MiB/s");
    expect(miBsUnit).toBeTruthy();
  });

  it("emits update:globalSpeedLimitMiBps when speed limit input changes", async () => {
    const { wrapper } = mountPanel({ globalSpeedLimitMiBps: 50 });

    // Find the speed limit text field by its data-unit attribute
    const speedField = wrapper.find('.ui-textfield-stub[data-unit="MiB/s"]');
    expect(speedField.exists()).toBe(true);

    // Trigger input on the textfield's input element
    const input = speedField.find(".ui-textfield__input-stub");
    await input.trigger("input");

    // Should have emitted with the input's value
    const emitted = wrapper.emitted("update:globalSpeedLimitMiBps");
    expect(emitted).toBeTruthy();
    expect(emitted![0][0]).toBe(50);
  });

  // ── Test 4: Chunk allocation switch ─────────────────────────
  it("chunk allocation switch toggles between adaptive and fixed", async () => {
    const { draft, wrapper } = mountPanel();

    // Label is rendered
    expect(wrapper.text()).toContain("settings.intelligentChunkAllocation");

    // Initially adaptive
    expect(draft.scheduler.chunkSizeStrategy).toBe("adaptive");

    // Find the switch stub
    const switchInput = wrapper.find(".ui-switch-stub");
    expect(switchInput.exists()).toBe(true);

    // Toggle to fixed
    await switchInput.setValue(false);
    expect(draft.scheduler.chunkSizeStrategy).toBe("fixed");

    // Toggle back to adaptive
    await switchInput.setValue(true);
    expect(draft.scheduler.chunkSizeStrategy).toBe("adaptive");
  });
});
