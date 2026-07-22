import { describe, it, expect, vi, beforeAll } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive } from "vue";
import SettingsBtPanel from "../../../components/settings/SettingsBtPanel.vue";
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
      '<span class="ui-textfield-stub"><input class="ui-textfield__input-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" /><span v-if="unit" class="ui-textfield__unit-stub">{{ unit }}</span></span>',
    props: ["modelValue", "min", "max", "disabled", "type", "unit", "placeholder", "step"],
  },
  UiSwitch: {
    template:
      '<input type="checkbox" class="ui-switch-stub" :checked="modelValue" @change="$emit(\'update:modelValue\', $event.target.checked)" />',
    props: ["modelValue", "label", "disabled"],
  },
  UiButton: {
    template: '<button class="ui-button-stub" :disabled="loading"><slot /></button>',
    props: ["type", "variant", "size", "icon", "loading"],
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

// ── Tests ──────────────────────────────────────────────────────────

describe("SettingsBtPanel", () => {
  it("renders the section header with btTitle", () => {
    const draft = reactive(createSettings());
    const wrapper = mount(SettingsBtPanel, {
      props: {
        draft,
        t: (key: string) => key,
        btSummary: "3 downloads active",
        btUploadLimitMiB: 0,
        isFetchingTrackerList: false,
        defaultTrackerListUrl: "https://example.com/trackers",
      },
      global: { stubs },
    });

    // Section header renders with title key
    expect(wrapper.text()).toContain("settings.btTitle");

    // At least one subgroup header renders (e.g. tracker group)
    expect(wrapper.text()).toContain("settings.btGroupTracker");

    // Network subgroup header renders
    expect(wrapper.text()).toContain("settings.btGroupNetwork");
  });
});
