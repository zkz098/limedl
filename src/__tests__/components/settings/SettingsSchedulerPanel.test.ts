import { describe, it, expect, vi, beforeAll } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive, nextTick } from "vue";
import SettingsSchedulerPanel from "../../../components/settings/SettingsSchedulerPanel.vue";
import type { AppSettings } from "../../../types/settings";

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
    pet: {
      enabled: false,
      scale: 1,
      opacity: 1,
      keepAliveWhenMainHidden: true,
      model: "default",
      transparentBackground: false,
    },
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
          { label: "Automatic", value: "automatic" },
          { label: "Traditional", value: "traditional" },
        ],
        adaptiveProfileOptions: [
          { label: "Conservative", value: "conservative" },
          { label: "Balanced", value: "balanced" },
          { label: "Aggressive", value: "aggressive" },
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
  // ── Test 1: Simple view by default ───────────────────────────
  it("renders in simple view with performance presets by default", () => {
    const { wrapper } = mountPanel();

    // Section header renders
    expect(wrapper.text()).toContain("settings.schedulerTitle");

    // Performance preset cards are visible
    expect(wrapper.text()).toContain("settings.performancePresetEnergySaver");
    expect(wrapper.text()).toContain("settings.performancePresetBalanced");
    expect(wrapper.text()).toContain("settings.performancePresetMaxSpeed");

    // Simple view shows speed limit and intelligent chunking
    expect(wrapper.text()).toContain("settings.globalSpeedLimit");
    expect(wrapper.text()).toContain("settings.intelligentChunking");

    // Custom view fields are hidden by default
    expect(wrapper.text()).not.toContain("settings.maxParallelThreads");
    expect(wrapper.text()).not.toContain("settings.allocationMode");
  });

  // ── Test 2: Switch to custom view and traditional mode ───────
  it("switches to custom view and shows traditional fields", async () => {
    const { draft, wrapper } = mountPanel();

    // Click custom view toggle
    const viewToggles = wrapper.findAll('[role="tab"]');
    const customBtn = viewToggles.find((btn) => btn.text().includes("settings.customView"));
    expect(customBtn).toBeTruthy();
    await customBtn!.trigger("click");
    await nextTick();

    // Now switch draft to traditional mode
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

  // ── Test 4: Performance presets ──────────────────────────────
  it("applies energy saver preset when clicked", async () => {
    const { draft, wrapper } = mountPanel();

    // Find the energy saver preset card (native radio inside a label)
    const presetCards = wrapper.findAll("label.performance-preset-card");
    const energyCard = presetCards.find((card) =>
      card.text().includes("settings.performancePresetEnergySaver"),
    );
    expect(energyCard).toBeTruthy();
    await energyCard!.find('input[type="radio"]').setValue(true);
    await nextTick();

    // Draft should be updated to energy saver values
    expect(draft.scheduler.automatic.maxParallelThreads).toBe(8);
    expect(draft.scheduler.automatic.maxThreadsPerTask).toBe(4);
    expect(draft.scheduler.automatic.minThreadsPerTask).toBe(2);
    expect(draft.scheduler.automatic.adaptiveProfile).toBe("conservative");
  });

  it("chunk switch uses intelligentChunking in simple view", async () => {
    const { draft, wrapper } = mountPanel();

    // Label is rendered
    expect(wrapper.text()).toContain("settings.intelligentChunking");

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
