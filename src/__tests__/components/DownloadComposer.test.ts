import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { setActivePinia, createPinia, storeToRefs } from "pinia";
import DownloadComposer from "../../components/limedl/DownloadComposer.vue";
import type { DownloadFormState } from "../../types/download";
import type { AppSettings } from "../../types/settings";
import { useDownloadStore } from "../../stores/download";

import { setupDownloadStoreMocks } from "../fixtures/download-store-mocks";
setupDownloadStoreMocks();

// Reflect the store's state refs + actions with the same `.value`-ref contract
// the download store reflects the state refs + actions with the same `.value`-ref contract
function useComposer() {
  const store = useDownloadStore();
  return { ...store, ...storeToRefs(store) };
}

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../i18n", () => ({
  useI18n: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (options && options.count !== undefined)
        return `${key} count=${JSON.stringify(options.count)}`;
      return key;
    },
  }),
  t: (key: string) => key,
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiButton: {
    template:
      '<button :disabled="disabled" :data-icon="icon" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "icon", "size", "variant", "type", "loading", "block"],
  },
  UiTextField: {
    template:
      '<input class="ui-textfield-stub" :value="modelValue" :placeholder="placeholder" :type="type" @input="$emit(\'update:modelValue\', $event.target.value)" @blur="$emit(\'blur\', $event)" />',
    props: ["modelValue", "placeholder", "type", "disabled"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" :disabled="disabled" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="opt in options" :key="opt.value" :value="opt.value">{{ opt.label }}</option></select>',
    props: ["modelValue", "options", "disabled", "placeholder"],
  },
  Transition: { template: "<div><slot /></div>" },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createSettings(overrides: Partial<AppSettings> = {}): AppSettings {
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
    ...overrides,
  };
}

describe("DownloadComposer", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  function mountWithForm(formOverrides?: Partial<DownloadFormState>) {
    const c = useComposer();
    if (formOverrides) {
      Object.assign(c.form.value, formOverrides);
    }
    return mount(DownloadComposer, {
      props: { settings: createSettings() },
      global: { stubs },
    });
  }

  // ── Rendering ──────────────────────────────────────────────
  it("renders source URL field", () => {
    const wrapper = mountWithForm();
    expect(wrapper.find(".composer-field__label").exists()).toBe(true);
    expect(wrapper.text()).toContain("composer.sourceUrl");
  });

  it("renders protocol toggle button", () => {
    const wrapper = mountWithForm({ kind: "http" });
    expect(wrapper.find(".composer-protocol").exists()).toBe(true);
    expect(wrapper.find(".composer-protocol__text").text()).toBe("tokens.http");
  });

  it("renders file name, save path, and submit button", () => {
    const wrapper = mountWithForm();
    expect(wrapper.text()).toContain("composer.fileName");
    expect(wrapper.text()).toContain("composer.savePath");
    expect(wrapper.text()).toContain("composer.start");
  });

  it("shows starting label when isStarting is true", () => {
    const c = useComposer();
    c.isStarting.value = true;
    const wrapper = mount(DownloadComposer, {
      props: { settings: createSettings() },
      global: { stubs },
    });
    const submitBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b: { text: () => string }) => b.text() === "composer.starting")!;
    expect(submitBtn).toBeDefined();
  });

  it("renders torrent selection button", () => {
    const wrapper = mountWithForm();
    expect(wrapper.text()).toContain("composer.chooseTorrent");
  });

  // ── Protocol detection ──────────────────────────────────────
  it.each<[string, string, "http" | "bt"]>([
    ["detects HTTP protocol from URL", "https://example.com/file.zip", "http"],
    ["detects magnet link as BT protocol", "magnet:?xt=urn:btih:ABC123", "bt"],
    ["detects .torrent URL as BT protocol", "https://example.com/file.torrent", "bt"],
    ["detects info hash as BT protocol", "deadbeefcafebabedeadbeefcafebabedeadbeef", "bt"],
  ])("%s", async (_title, url, expectedKind) => {
    const wrapper = mountWithForm();
    const c = useComposer();
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue(url);
    await nextTick();
    expect(c.form.value.kind).toBe(expectedKind);
  });

  it("shows BT protocol label when kind is bt", () => {
    const wrapper = mountWithForm({ kind: "bt" });
    expect(wrapper.find(".composer-protocol__text").text()).toBe("tokens.bt");
  });

  it("toggles protocol kind when protocol button is clicked", async () => {
    const wrapper = mountWithForm({ kind: "http" });
    const c = useComposer();
    await wrapper.find(".composer-protocol").trigger("click");
    expect(c.form.value.kind).toBe("bt");
  });

  it.each<[string, string, string, string]>([
    ["auto-extracts file name from HTTP URL", "https://example.com/myfile.zip", "", "myfile.zip"],
    ["auto-extracts file name from magnet dn parameter", "magnet:?xt=urn:btih:ABC&dn=ubuntu.iso", "", "ubuntu.iso"],
    ["does not overwrite existing file name when URL changes", "https://example.com/newfile.zip", "existing.zip", "existing.zip"],
  ])("%s", async (_title, url, initialFileName, expectedFileName) => {
    const wrapper = mountWithForm({ fileName: initialFileName });
    const c = useComposer();
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue(url);
    await nextTick();
    expect(c.form.value.fileName).toBe(expectedFileName);
  });

  // ── URL validation ──────────────────────────────────────────
  it.each<[string, "http" | "bt", string, boolean]>([
    ["shows error when URL is not valid for HTTP kind", "http", "invalid-url", true],
    ["does not show error for valid HTTP URL", "http", "https://example.com/file.zip", false],
    ["shows error for HTTP URL without protocol", "http", "example.com/file.zip", true],
  ])("%s", async (_title, kind, url, expectInvalid) => {
    const wrapper = mountWithForm({ kind, url });
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    if (expectInvalid) {
      expect(wrapper.text()).toContain("composer.urlInvalid");
    } else {
      expect(wrapper.text()).not.toContain("composer.urlInvalid");
    }
  });

  it("does not show error for magnet URL with BT kind", async () => {
    const wrapper = mountWithForm({ kind: "bt", url: "magnet:?xt=urn:btih:ABC" });
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    expect(wrapper.text()).not.toContain("composer.urlInvalid");
  });

  it("shows error for invalid HTTP URL in BT kind", async () => {
    const wrapper = mountWithForm({ kind: "bt", url: "http://" });
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("composer.urlInvalid");
  });

  // ── Advanced options panel ──────────────────────────────────
  it("advanced options panel is hidden by default", () => {
    const wrapper = mountWithForm();
    const panel = wrapper.find(".composer-advanced__panel");
    expect(panel.exists()).toBe(true);
    expect(panel.attributes("style")).toContain("display: none");
  });

  it("toggles advanced options visibility", async () => {
    const wrapper = mountWithForm();
    const panel = wrapper.find(".composer-advanced__panel");
    expect(panel.attributes("style")).toContain("display: none");
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(panel.attributes("style")).toBe("");
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(panel.attributes("style")).toContain("display: none");
  });

  it("shows thread strategy and thread count selects in advanced", async () => {
    const wrapper = mountWithForm();
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.threadStrategy");
    expect(wrapper.text()).toContain("composer.threadCount");
  });

  it("shows adaptive hint when threadMode is adaptive", async () => {
    const wrapper = mountWithForm({ kind: "http", threadMode: "adaptive" });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.adaptiveHint");
  });

  it("shows BT-specific fields in advanced when kind is bt", async () => {
    const wrapper = mountWithForm({ kind: "bt", url: "magnet:?xt=urn:btih:ABC" });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.btDownloadLimit");
    expect(wrapper.text()).toContain("composer.btUploadLimit");
    expect(wrapper.text()).toContain("composer.btHint");
  });

  it("does not show BT fields in advanced when kind is http", async () => {
    const wrapper = mountWithForm({ kind: "http" });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).not.toContain("composer.btDownloadLimit");
    expect(wrapper.text()).not.toContain("composer.btUploadLimit");
  });

  it("shows fixedHint when threadMode is fixed", async () => {
    const wrapper = mountWithForm({ kind: "http", threadMode: "fixed" });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.fixedHint");
  });

  it("emits submit when form is submitted", async () => {
    const wrapper = mountWithForm();
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("submit")).toBeDefined();
    expect(wrapper.emitted("submit")).toHaveLength(1);
  });

  it("applies is-invalid class to source container when urlError is set", async () => {
    const wrapper = mountWithForm({ kind: "http", url: "not-a-url" });
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    await wrapper.find(".composer-protocol").trigger("click");
    await nextTick();
    const sourceContainer = wrapper.find(".composer-source");
    expect(sourceContainer.classes()).toContain("is-invalid");
  });

  it("shows traditional hint when scheduler mode is traditional", async () => {
    const settings = createSettings({
      scheduler: {
        ...createSettings().scheduler,
        mode: "traditional",
        traditional: { maxParallelTasks: 3 },
        automatic: {
          maxParallelThreads: 4,
          maxThreadsPerTask: 2,
          minThreadsPerTask: 1,
          adaptiveProfile: "balanced",
        },
      },
    });
    const wrapper = mount(DownloadComposer, {
      props: { settings },
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.traditionalHint");
  });
});
