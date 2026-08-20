import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { reactive, nextTick } from "vue";
import SettingsIoBaselinePanel from "../../components/settings/SettingsIoBaselinePanel.vue";
import type { AppSettings, IoBaselineSettings } from "../../types/settings";

// ── Hoisted mocks ────────────────────────────────────────────────────

const mockDetectAllDiskTypes = vi.hoisted(() => vi.fn());
const mockFormatBytes = vi.hoisted(() => vi.fn());

vi.mock("../../lib/tauri/settings-api", () => ({
  detectAllDiskTypes: mockDetectAllDiskTypes,
}));

vi.mock("../../lib/download-format", () => ({
  formatBytes: mockFormatBytes,
}));

// ── Stubs ────────────────────────────────────────────────────────────

const stubs = {
  SettingsSection: {
    template:
      '<div class="settings-section-stub"><span v-if="title" class="settings-section-stub__title">{{ title }}</span><slot /></div>',
    props: ["title", "icon"],
  },
  SettingsField: {
    template:
      '<div class="settings-field-stub"><span v-if="label" class="settings-field__label">{{ label }}</span><div class="settings-field-stub__slot"><slot /></div></div>',
    props: ["label", "infoTooltip", "wide", "hint"],
  },
  UiSwitch: {
    template:
      '<input type="checkbox" class="ui-switch-stub" :checked="modelValue" :disabled="disabled" @change="$emit(\'update:modelValue\', $event.target.checked)" />',
    props: ["modelValue", "disabled"],
  },
  UiTextField: {
    template:
      '<input class="ui-textfield-stub" :value="modelValue" :type="type" :disabled="disabled" :min="min" :max="max" :data-unit="unit ?? undefined" @input="$emit(\'update:modelValue\', type === \'number\' ? ($event.target.value === \'\' ? null : Number($event.target.value)) : $event.target.value)" />',
    props: ["modelValue", "type", "disabled", "min", "max", "unit", "placeholder"],
  },
};

// ── Helpers ──────────────────────────────────────────────────────────

function createDraft(overrides?: Partial<IoBaselineSettings>): AppSettings {
  const base: AppSettings = {
    // Provide minimal required AppSettings fields beyond ioBaseline
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
      showDetailInfo: false,
      showHeatmap: false,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: [],
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
      enableNatpmp: false,
      enableIpv6: false,
      enablePex: true,
      enableLsd: true,
      enableUtp: true,
      enableFastExtension: true,
      enableHolepunch: true,
      enableWebSeed: true,
      enableSuperSeeding: false,
      preallocateMode: "none",
      encryptionMode: "enabled",
      maxDownloads: 5,
      maxSeeds: 2,
      maxTorrents: 10,
      activeLimit: 15,
      globalDownloadRateLimit: 0,
      globalUploadRateLimit: 0,
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
      bufferLimitMb: 1024,
      gameModeBufferMb: 128,
      maxParallelHdd: 4,
      gameModeMaxParallel: 1,
      hddBufferEnabled: true,
      diskTypeOverrides: {},
      ssdWriteCombineMb: 0,
      ...overrides,
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    doubleClick: { onCompleted: "none", onUncompleted: "none" },
    speedLimitSchedule: [],
  };
  return base;
}

function mountPanel(props: Record<string, unknown> = {}) {
  const ioBaselineValue = props.ioBaseline;
  const ioBaselineOverrides: Partial<IoBaselineSettings> =
    ioBaselineValue != null && typeof ioBaselineValue === "object" ? ioBaselineValue : {};
  const draft = reactive(createDraft(ioBaselineOverrides));

  // See ioBaseline from other props so we don't pass it down
  const { ioBaseline: _ioBaseline, ...otherProps } = props;

  return mount(SettingsIoBaselinePanel, {
    props: {
      draft,
      t: (key: string) => key,
      gameMode: false,
      bufferUsageBytes: 0,
      bufferLimitBytes: 0,
      activeSlots: 0,
      maxSlots: 0,
      queuedCount: 0,
      ...otherProps,
    },
    global: { stubs },
    attachTo: document.body,
  });
}

/** Return the UiTextField stubs in DOM order matching the template layout:
 *  0: bufferLimit, 1: gameModeBuffer, 2: maxParallelHdd, 3: gameModeMaxParallel */
function getTextFields(wrapper: ReturnType<typeof mountPanel>) {
  return wrapper.findAll(".ui-textfield-stub");
}

/** Return the UiSwitch stub */
function getSwitch(wrapper: ReturnType<typeof mountPanel>) {
  return wrapper.find(".ui-switch-stub");
}

/** Extract the draft object from wrapper props with proper typing */
function getDraft(wrapper: ReturnType<typeof mountPanel>): AppSettings {
  const props = wrapper.props();
  // props.draft is strongly typed via the mount call
  return props.draft;
}

// ── Tests ────────────────────────────────────────────────────────────

describe("SettingsIoBaselinePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default mock: no HDD detected
    mockDetectAllDiskTypes.mockResolvedValue({ "C:\\": "ssd" });
    mockFormatBytes.mockImplementation((v: number | null | undefined) => {
      if (typeof v !== "number" || Number.isNaN(v)) return "—";
      return `${v}B`;
    });
  });

  // ── 1. HDD detection display ─────────────────────────────────

  describe("HDD detection display", () => {
    it("shows warning banner when no HDD detected and HDD buffer is enabled", async () => {
      // Return only SSDs — no HDD
      mockDetectAllDiskTypes.mockResolvedValue({ "C:\\": "ssd" });
      // Start with buffer enabled; auto-disable will fire after scan, so we
      // re-enable it afterward to test the warning condition.
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      // After scan with no HDDs, auto-disable set hddBufferEnabled to false.
      // Now re-enable it to trigger the warning banner:
      getDraft(wrapper).ioBaseline.hddBufferEnabled = true;
      await nextTick();

      expect(wrapper.find(".io-warning-banner").exists()).toBe(true);
      expect(wrapper.find(".io-warning-banner").attributes("role")).toBe("alert");
      expect(wrapper.text()).toContain("settings.ioBaseline.hddBufferNoHddWarning");
    });

    it("shows info banner when no HDD detected and HDD buffer is disabled", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({ "C:\\": "ssd" });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: false } });
      await flushPromises();
      await nextTick();

      expect(wrapper.find(".io-info-banner").exists()).toBe(true);
      expect(wrapper.text()).toContain("settings.ioBaseline.noHddDetectedInfo");
    });

    it("hides both banners when HDD is detected", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({ "D:\\": "hdd" });
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      expect(wrapper.find(".io-warning-banner").exists()).toBe(false);
      expect(wrapper.find(".io-info-banner").exists()).toBe(false);
    });

    it("hides both banners when detectAllDiskTypes throws (safe fallback)", async () => {
      mockDetectAllDiskTypes.mockRejectedValue(new Error("scan failed"));
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      expect(wrapper.find(".io-warning-banner").exists()).toBe(false);
      expect(wrapper.find(".io-info-banner").exists()).toBe(false);
    });

    it("shows info banner for empty disk types map", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({});
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: false } });
      await flushPromises();
      await nextTick();

      expect(wrapper.find(".io-info-banner").exists()).toBe(true);
      expect(wrapper.text()).toContain("settings.ioBaseline.noHddDetectedInfo");
    });

    it("no banners displayed before scanAllDrives completes (hasHdd === null)", () => {
      // Promise that never resolves during initial tick — keep scan pending
      mockDetectAllDiskTypes.mockReturnValue(new Promise(() => {}));
      const wrapper = mountPanel();
      // hasHdd is still null at this point
      expect(wrapper.find(".io-warning-banner").exists()).toBe(false);
      expect(wrapper.find(".io-info-banner").exists()).toBe(false);
    });
  });

  // ── 2. Buffer limit input ───────────────────────────────────

  describe("Buffer limit input", () => {
    it("renders current bufferLimitMb value", async () => {
      const wrapper = mountPanel({ ioBaseline: { bufferLimitMb: 2048 } });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      const inputEl = inputs[0].element;
      const value = inputEl instanceof HTMLInputElement ? inputEl.value : "";
      expect(value).toBe("2048");
    });

    it.each([
      ["clamps value to minimum 64 when set lower", "10", 64],
      ["clamps value to maximum 32768 when set higher", "50000", 32768],
      ["truncates non-integer values", "123.89", 123],
    ])("%s", async (_title, input, expected) => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      await inputs[0].setValue(input);
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.bufferLimitMb).toBe(expected);
    });

    it("renders min/max attributes on the input", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      // bufferLimit: min=64, max=32768
      expect(inputs[0].attributes("min")).toBe("64");
      expect(inputs[0].attributes("max")).toBe("32768");
    });

    it("uses null coalescing fallback when value is null", async () => {
      const wrapper = mountPanel({ ioBaseline: { bufferLimitMb: 1024 } });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      // Emit null from the stub (empty number input)
      await inputs[0].setValue("");
      await nextTick();

      const draft = getDraft(wrapper);
      // null → fallback 1024 → clamped (within range) → 1024
      expect(draft.ioBaseline.bufferLimitMb).toBe(1024);
    });
  });

  // ── 3. Game mode buffer input ───────────────────────────────

  describe("Game mode buffer input", () => {
    it("is disabled when gameMode is false", async () => {
      const wrapper = mountPanel({ gameMode: false });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      // Second input is gameModeBuffer
      expect(inputs[1].attributes("disabled")).toBeDefined();
    });

    it("is enabled when gameMode is true", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[1].attributes("disabled")).toBeUndefined();
    });

    it.each([
      ["clamps to minimum 16", "1", 16],
      ["clamps to maximum 4096", "5000", 4096],
      ["truncates non-integer values", "99.99", 99],
    ])("%s", async (_title, input, expected) => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      await inputs[1].setValue(input);
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.gameModeBufferMb).toBe(expected);
    });

    it("renders min/max attributes on the input", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[1].attributes("min")).toBe("16");
      expect(inputs[1].attributes("max")).toBe("4096");
    });
  });

  // ── 4. Max parallel HDD input ───────────────────────────────

  describe("Max parallel HDD input", () => {
    it.each([
      ["clamps to minimum 1", "0", 1],
      ["clamps to maximum 16", "20", 16],
      ["truncates non-integer values", "7.8", 7],
    ])("%s", async (_title, input, expected) => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      await inputs[2].setValue(input);
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.maxParallelHdd).toBe(expected);
    });

    it("renders min/max attributes on the input", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[2].attributes("min")).toBe("1");
      expect(inputs[2].attributes("max")).toBe("16");
    });
  });

  // ── 5. Game mode max parallel input ─────────────────────────

  describe("Game mode max parallel input", () => {
    it("is disabled when gameMode is false", async () => {
      const wrapper = mountPanel({ gameMode: false });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      // Fourth input is gameModeMaxParallel
      expect(inputs[3].attributes("disabled")).toBeDefined();
    });

    it("is enabled when gameMode is true", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[3].attributes("disabled")).toBeUndefined();
    });

    it("clamps to minimum 1", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      await inputs[3].setValue("0");
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.gameModeMaxParallel).toBe(1);
    });

    it("clamps to maximum 8", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      await inputs[3].setValue("10");
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.gameModeMaxParallel).toBe(8);
    });

    it("renders min/max attributes on the input", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[3].attributes("min")).toBe("1");
      expect(inputs[3].attributes("max")).toBe("8");
    });
  });

  // ── 6. I/O status display ───────────────────────────────────

  describe("I/O status display", () => {
    it("shows bufferUsageText with formatted bytes", async () => {
      mockFormatBytes.mockImplementation((v: number | null | undefined) => {
        if (typeof v !== "number" || Number.isNaN(v)) return "—";
        if (v >= 1073741824) return `${(v / 1073741824).toFixed(1)} GB`;
        if (v >= 1048576) return `${(v / 1048576).toFixed(1)} MB`;
        if (v >= 1024) return `${(v / 1024).toFixed(1)} KB`;
        return `${v} B`;
      });

      const wrapper = mountPanel({
        bufferUsageBytes: 52428800, // 50 MB
        bufferLimitBytes: 1073741824, // 1 GB
      });
      await flushPromises();
      await nextTick();

      expect(wrapper.text()).toContain("50.0 MB / 1.0 GB");
    });

    it("shows slotUsageText with correct formatting", async () => {
      const wrapper = mountPanel({
        activeSlots: 3,
        maxSlots: 8,
        queuedCount: 12,
      });
      await flushPromises();
      await nextTick();

      expect(wrapper.text()).toContain("3 / 8 (queued: 12)");
    });

    it("calls formatBytes with bufferUsageBytes and bufferLimitBytes", async () => {
      mockFormatBytes.mockReturnValue("X");

      mountPanel({
        bufferUsageBytes: 12345,
        bufferLimitBytes: 67890,
      });
      await flushPromises();
      await nextTick();

      expect(mockFormatBytes).toHaveBeenCalledWith(12345);
      expect(mockFormatBytes).toHaveBeenCalledWith(67890);
    });

    it("shows placeholder for null/NaN buffer values", async () => {
      mockFormatBytes.mockImplementation((v: number | null | undefined) => {
        if (typeof v !== "number" || Number.isNaN(v)) return "—";
        return `${v} B`;
      });

      const wrapper = mountPanel({
        bufferUsageBytes: NaN,
        bufferLimitBytes: Number(null),
      });
      await flushPromises();
      await nextTick();

      expect(wrapper.text()).toContain("—");
    });

    it("wraps status display in SettingsField with correct label", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      expect(wrapper.text()).toContain("settings.ioBaseline.status");
      expect(wrapper.text()).toContain("settings.ioBaseline.bufferUsage");
      expect(wrapper.text()).toContain("settings.ioBaseline.activeSlots");
    });
  });

  // ── 7. HDD buffer toggle ──────────────────────────────────

  describe("HDD buffer toggle", () => {
    it("toggles hddBufferEnabled to false when clicked from on", async () => {
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      const sw = getSwitch(wrapper);
      await sw.setValue(false);
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(false);
    });

    it("toggles hddBufferEnabled to true when clicked from off", async () => {
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: false } });
      await flushPromises();
      await nextTick();

      const sw = getSwitch(wrapper);
      await sw.setValue(true);
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(true);
    });

    it("auto-disables HDD buffer when no HDD detected and buffer was enabled", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({ "C:\\": "ssd" });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(false);
    });

    it("does not auto-disable HDD buffer when HDD is detected", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({ "D:\\": "hdd" });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(true);
    });

    it("does not auto-disable when hddBufferEnabled is already false", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({ "C:\\": "ssd" });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: false } });
      await flushPromises();
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(false);
    });
  });

  // ── 8. Section and field rendering ─────────────────────────

  describe("Section and field rendering", () => {
    it("renders SettingsSection with correct title and icon", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const section = wrapper.find(".settings-section-stub");
      expect(section.exists()).toBe(true);
      expect(wrapper.text()).toContain("settings.ioBaseline.title");
    });

    it("renders all SettingsField labels", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      expect(wrapper.text()).toContain("settings.ioBaseline.hddBufferToggle");
      expect(wrapper.text()).toContain("settings.ioBaseline.bufferLimit");
      expect(wrapper.text()).toContain("settings.ioBaseline.gameModeBuffer");
      expect(wrapper.text()).toContain("settings.ioBaseline.maxParallelHdd");
      expect(wrapper.text()).toContain("settings.ioBaseline.gameModeMaxParallel");
      expect(wrapper.text()).toContain("settings.ioBaseline.status");
    });

    it("renders bufferLimit with MB unit", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[0].attributes("data-unit")).toBe("MB");
    });

    it("renders gameModeBuffer with MB unit", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[1].attributes("data-unit")).toBe("MB");
    });

    it("renders maxParallelHdd without unit", async () => {
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[2].attributes("data-unit")).toBeUndefined();
    });

    it("renders gameModeMaxParallel without unit", async () => {
      const wrapper = mountPanel({ gameMode: true });
      await flushPromises();
      await nextTick();

      const inputs = getTextFields(wrapper);
      expect(inputs[3].attributes("data-unit")).toBeUndefined();
    });
  });

  // ── 9. Edge cases ──────────────────────────────────────────

  describe("Edge cases", () => {
    it("detectAllDiskTypes rejection falls back to hasHdd=true", async () => {
      mockDetectAllDiskTypes.mockRejectedValue(new Error("access denied"));
      const wrapper = mountPanel();
      await flushPromises();
      await nextTick();

      // hasHdd = true → no banners, no auto-disable
      expect(wrapper.find(".io-warning-banner").exists()).toBe(false);
      expect(wrapper.find(".io-info-banner").exists()).toBe(false);
    });

    it("detectAllDiskTypes resolves with mixed drives (ssd + hdd) → hasHdd=true", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({
        "C:\\": "ssd",
        "D:\\": "hdd",
        "E:\\": "ssd",
      });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      // HDD present, buffer stays enabled
      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(true);
      expect(wrapper.find(".io-warning-banner").exists()).toBe(false);
    });

    it("detectAllDiskTypes resolves with all SSDs → hasHdd=false → auto-disable", async () => {
      mockDetectAllDiskTypes.mockResolvedValue({
        "C:\\": "ssd",
        "D:\\": "ssd",
      });
      const wrapper = mountPanel({ ioBaseline: { hddBufferEnabled: true } });
      await flushPromises();
      await nextTick();

      const draft = getDraft(wrapper);
      expect(draft.ioBaseline.hddBufferEnabled).toBe(false);
    });

    it("providing undefined ioBaseline fields falls back to defaults via ??", async () => {
      // On mount, no banners because hasHdd is still null.
      // Then scan resolves to no HDD, auto-disable fires (hddBufferEnabled was true).
      // banner: info shown because hddBufferEnabled becomes false.
      mockDetectAllDiskTypes.mockResolvedValue({});
      const wrapper = mountPanel({
        ioBaseline: {
          bufferLimitMb: 1024,
          gameModeBufferMb: 128,
          maxParallelHdd: 4,
          gameModeMaxParallel: 1,
          hddBufferEnabled: true,
          diskTypeOverrides: {},
          ssdWriteCombineMb: 0,
        },
      });
      await flushPromises();
      await nextTick();

      // hddBufferEnabled was auto-disabled → info banner
      expect(wrapper.find(".io-info-banner").exists()).toBe(true);
    });

    it("UiSwitch model-value uses ?? true fallback", () => {
      // When hddBufferEnabled is undefined, the switch still shows as checked
      // (hasHdd is null, so no banner either)
      const wrapper = mountPanel({
        ioBaseline: { hddBufferEnabled: undefined },
      });

      const sw = getSwitch(wrapper);
      const swEl = sw.element;
      const checked = swEl instanceof HTMLInputElement ? swEl.checked : false;
      expect(checked).toBe(true);
    });
  });
});
