import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import DownloadComposer from "../../components/limedl/DownloadComposer.vue";
import type { DownloadFormState } from "../../types/download";
import type { AppSettings } from "../../types/settings";

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
  // Stub Transition so v-show behavior works correctly in tests
  Transition: { template: "<div><slot /></div>" },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createForm(overrides: Partial<DownloadFormState> = {}): DownloadFormState {
  return {
    kind: "http",
    url: "",
    destinationDir: "",
    fileName: "",
    userAgent: "",
    threadMode: "adaptive",
    threadCount: null,
    maxRetries: null,
    checksum: "none",
    downloadLimitBps: null,
    uploadLimitBps: null,
    ...overrides,
  };
}

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
    },
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    ...overrides,
  };
}

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    form: createForm(),
    isStarting: false,
    isPickingDirectory: false,
    isPickingTorrent: false,
    settings: createSettings(),
    ...overrides,
  };
}

describe("DownloadComposer", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders source URL field", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".composer-field__label").exists()).toBe(true);
    expect(wrapper.text()).toContain("composer.sourceUrl");
  });

  it("renders protocol toggle button", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".composer-protocol").exists()).toBe(true);
    expect(wrapper.find(".composer-protocol__text").text()).toBe("tokens.http");
  });

  it("renders file name, save path, and submit button", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("composer.fileName");
    expect(wrapper.text()).toContain("composer.savePath");
    expect(wrapper.text()).toContain("composer.start");
  });

  it("shows starting label when isStarting is true", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps({ isStarting: true }),
      global: { stubs },
    });
    const submitBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "composer.starting")!;
    expect(submitBtn).toBeDefined();
  });

  it("renders torrent selection button", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("composer.chooseTorrent");
  });

  // ── Protocol detection ──────────────────────────────────────
  it("detects HTTP protocol from URL", async () => {
    const form = createForm();
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("https://example.com/file.zip");
    await nextTick();
    expect(form.kind).toBe("http");
  });

  it("detects magnet link as BT protocol", async () => {
    const form = createForm();
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("magnet:?xt=urn:btih:ABC123&dn=test.torrent");
    await nextTick();
    expect(form.kind).toBe("bt");
  });

  it("detects .torrent URL as BT protocol", async () => {
    const form = createForm();
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("https://example.com/file.torrent");
    await nextTick();
    expect(form.kind).toBe("bt");
  });

  it("detects info hash as BT protocol", async () => {
    const form = createForm();
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("ABC123DEF456ABC123DEF456ABC123DEF456ABC1");
    await nextTick();
    expect(form.kind).toBe("bt");
  });

  it("shows BT protocol label when kind is bt", async () => {
    const form = createForm({ kind: "bt", url: "magnet:?xt=urn:btih:ABC" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    expect(wrapper.find(".composer-protocol__text").text()).toBe("tokens.bt");
  });

  // ── Protocol toggle ─────────────────────────────────────────
  it("toggles protocol kind when protocol button is clicked", async () => {
    const form = createForm({ kind: "http" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    await wrapper.find(".composer-protocol").trigger("click");
    expect(form.kind).toBe("bt");
  });

  // ── File name auto-extraction ───────────────────────────────
  it("auto-extracts file name from HTTP URL", async () => {
    const form = createForm({ fileName: "" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("https://example.com/my-document.pdf");
    await nextTick();
    expect(form.fileName).toBe("my-document.pdf");
  });

  it("auto-extracts file name from magnet dn parameter", async () => {
    const form = createForm({ fileName: "" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("magnet:?xt=urn:btih:ABC&dn=ubuntu-24.04.iso");
    await nextTick();
    expect(form.fileName).toBe("ubuntu-24.04.iso");
  });

  it("does not overwrite existing file name when URL changes", async () => {
    const form = createForm({ fileName: "custom-name.zip" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.setValue("https://example.com/actual-name.zip");
    await nextTick();
    // fileName already set, so it should stay as "custom-name.zip"
    expect(form.fileName).toBe("custom-name.zip");
  });

  // ── URL validation ──────────────────────────────────────────
  it("shows error when URL is not valid for HTTP kind", async () => {
    const form = createForm({ kind: "http", url: "not-a-url" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    // Trigger blur validation
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-field__error").exists()).toBe(true);
    expect(wrapper.text()).toContain("composer.urlInvalid");
  });

  it("does not show error for valid HTTP URL", async () => {
    const form = createForm({ kind: "http", url: "https://example.com/file.zip" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-field__error").exists()).toBe(false);
  });

  it("shows error for HTTP URL without protocol", async () => {
    const form = createForm({ kind: "http", url: "ftp://example.com/file.zip" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-field__error").exists()).toBe(true);
  });

  it("does not show error for magnet URL with BT kind", async () => {
    const form = createForm({ kind: "bt", url: "magnet:?xt=urn:btih:ABC" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-field__error").exists()).toBe(false);
  });

  it("shows error for invalid HTTP URL in BT kind", async () => {
    const form = createForm({ kind: "bt", url: "http://not valid" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-field__error").exists()).toBe(true);
  });

  // ── Advanced options ────────────────────────────────────────
  it("advanced options panel is hidden by default", () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    // v-show renders the element but hides it
    const panel = wrapper.find(".composer-advanced__panel");
    expect(panel.exists()).toBe(true);
    expect(panel.isVisible()).toBe(false);
  });

  it("toggles advanced options visibility", async () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });

    // Initially hidden (v-show="false")
    let panel = wrapper.find(".composer-advanced__panel");
    expect(panel.exists()).toBe(true);
    // v-show applies display:none when false
    expect(panel.attributes("style")).toContain("display: none");

    // Click to expand
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    panel = wrapper.find(".composer-advanced__panel");
    // When v-show is true, the style should NOT contain display: none
    const style = panel.attributes("style");
    if (style) {
      expect(style).not.toContain("display: none");
    }

    // Click to collapse
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    panel = wrapper.find(".composer-advanced__panel");
    expect(panel.attributes("style")).toContain("display: none");
  });

  it("shows thread strategy and thread count selects in advanced", async () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.threadStrategy");
    expect(wrapper.text()).toContain("composer.threadCount");
  });

  it("shows adaptive hint when threadMode is adaptive", async () => {
    const form = createForm({ kind: "http", threadMode: "adaptive" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.adaptiveHint");
  });

  it("shows BT-specific fields in advanced when kind is bt", async () => {
    const form = createForm({ kind: "bt", url: "magnet:?xt=urn:btih:ABC" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.btDownloadLimit");
    expect(wrapper.text()).toContain("composer.btUploadLimit");
    expect(wrapper.text()).toContain("composer.btHint");
  });

  it("does not show BT fields in advanced when kind is http", async () => {
    const form = createForm({ kind: "http" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).not.toContain("composer.btDownloadLimit");
    expect(wrapper.text()).not.toContain("composer.btUploadLimit");
  });

  it("shows fixedHint when threadMode is fixed", async () => {
    const form = createForm({ kind: "http", threadMode: "fixed" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.fixedHint");
  });

  // ── Events ──────────────────────────────────────────────────
  it("emits submit when form is submitted", async () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("submit")).toBeTruthy();
  });

  it("emits pickDirectory when directory button is clicked", async () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    const dirBtn = wrapper.findAll("button.ui-button-stub").find((b) => {
      return b.attributes("data-icon") === "i-ri-folder-open-line";
    });
    if (dirBtn) {
      await dirBtn.trigger("click");
      expect(wrapper.emitted("pickDirectory")).toBeTruthy();
    }
  });

  it("emits pickTorrent when torrent button is clicked", async () => {
    const wrapper = mount(DownloadComposer, {
      props: createProps(),
      global: { stubs },
    });
    // Find button with text "composer.chooseTorrent"
    const torrentBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "composer.chooseTorrent")!;
    await torrentBtn.trigger("click");
    expect(wrapper.emitted("pickTorrent")).toBeTruthy();
  });

  // ── URL error styling ───────────────────────────────────────
  it("applies is-invalid class to source container when urlError is set", async () => {
    const form = createForm({ kind: "http", url: "bad-url" });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ form }),
      global: { stubs },
    });
    const urlInput = wrapper.find(".ui-textfield-stub");
    await urlInput.trigger("blur");
    await nextTick();
    expect(wrapper.find(".composer-source.is-invalid").exists()).toBe(true);
  });

  // ── Traditional scheduler mode ──────────────────────────────
  it("shows traditional hint when scheduler mode is traditional", async () => {
    const settings = createSettings({
      scheduler: { ...createSettings().scheduler, mode: "traditional" },
    });
    const wrapper = mount(DownloadComposer, {
      props: createProps({ settings }),
      global: { stubs },
    });
    await wrapper.find(".composer-advanced__trigger").trigger("click");
    expect(wrapper.text()).toContain("composer.traditionalHint");
  });
});
