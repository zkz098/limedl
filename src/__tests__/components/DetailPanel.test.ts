import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import DetailPanel from "../../components/limedl/DetailPanel.vue";
import type { DownloadSummary } from "../../types/download";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
  t: (key: string) => key,
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiBadge: {
    template: '<span class="ui-badge-stub" :class="`tone-${tone}`"><slot /></span>',
    props: ["tone", "size"],
  },
  UiButton: {
    template:
      '<button :disabled="disabled" :data-icon="icon || iconRight" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "icon", "iconRight", "size", "variant", "type", "loading"],
  },
  DownloadInspector: {
    template: '<div class="download-inspector-stub" />',
    props: ["selectedOverview", "selectedSnapshot", "showDetailInfo"],
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createOverview(overrides: Record<string, unknown> = {}): DownloadSummary {
  return {
    id: "test-1",
    kind: "http",
    state: "downloading",
    fileName: "test-file.zip",
    url: "https://example.com/test-file.zip",
    destinationPath: "/tmp/test-file.zip",
    totalBytes: 1024 * 1024 * 100,
    downloadedBytes: 1024 * 1024 * 25,
    connectionCount: 4,
    threadMode: "adaptive",
    requestedThreadCount: null,
    desiredThreadCount: null,
    allocatedThreadCount: null,
    adaptiveProfile: null,
    threadNote: null,
    speedBytesPerSecond: 1024 * 500,
    etaSeconds: 150,
    uploadedBytes: null,
    uploadSpeedBytesPerSecond: null,
    peerCount: null,
    uploadStatus: null,
    infoHash: null,
    error: null,
    cdnAccelerated: false,
    createdAtMs: 1000,
    ...overrides,
  } satisfies DownloadSummary;
}

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    selectedOverview: null,
    selectedSnapshot: null,
    selectedId: null,
    canPause: false,
    canResume: false,
    canCancel: false,
    actionName: "",
    isRefreshingStatus: false,
    showDetailInfo: true,
    ...overrides,
  };
}

describe("DetailPanel", () => {
  // ── Rendering (no selection) ────────────────────────────────
  it("renders with no selection text when selectedOverview is null", () => {
    const wrapper = mount(DetailPanel, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("detail.noSelection");
    expect(wrapper.find(".detail-panel__empty").exists()).toBe(true);
    expect(wrapper.find(".download-inspector-stub").exists()).toBe(false);
  });

  it("shows selectPrompt in the body when no overview", () => {
    const wrapper = mount(DetailPanel, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".detail-panel__empty").text()).toContain("detail.selectPrompt");
  });

  // ── Rendering (with selection) ──────────────────────────────
  it("renders file name when selectedOverview is provided", () => {
    const overview = createOverview({ fileName: "important-doc.pdf" });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canPause: true, canCancel: true }),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("important-doc.pdf");
    expect(wrapper.find(".download-inspector-stub").exists()).toBe(true);
    expect(wrapper.find(".detail-panel__empty").exists()).toBe(false);
  });

  it("renders state badge with correct tone for downloading state", () => {
    const overview = createOverview({ state: "downloading" });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    const badge = wrapper.find(".ui-badge-stub");
    expect(badge.exists()).toBe(true);
    expect(badge.classes()).toContain("tone-info");
    expect(badge.text()).toBe("states.downloading");
  });

  it("renders state badge with success tone for completed state", () => {
    const overview = createOverview({ state: "completed" });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    expect(wrapper.find(".ui-badge-stub").classes()).toContain("tone-success");
  });

  it("renders state badge with danger tone for failed state", () => {
    const overview = createOverview({ state: "failed" });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    expect(wrapper.find(".ui-badge-stub").classes()).toContain("tone-danger");
  });

  it("renders state badge with warning tone for paused state", () => {
    const overview = createOverview({ state: "paused" });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    expect(wrapper.find(".ui-badge-stub").classes()).toContain("tone-warning");
  });

  it("renders CDN badge when cdnAccelerated is true", () => {
    const overview = createOverview({ cdnAccelerated: true });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    const cdnBadge = wrapper.find(".detail-panel__cdn");
    expect(cdnBadge.exists()).toBe(true);
    expect(cdnBadge.text()).toContain("inspector.cdnAccelerated");
  });

  it("does not render CDN badge when cdnAccelerated is false", () => {
    const overview = createOverview({ cdnAccelerated: false });
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    expect(wrapper.find(".detail-panel__cdn").exists()).toBe(false);
  });

  // ── Action buttons ──────────────────────────────────────────
  it("renders refresh button when overview is selected", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const refreshBtn = buttons.find((b) => b.text() === "common.refresh");
    expect(refreshBtn).toBeDefined();
  });

  it("emits refresh when refresh button is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const refreshBtn = buttons.find((b) => b.text() === "common.refresh")!;
    await refreshBtn.trigger("click");
    // The stub button emits click which triggers the parent @click handler
    expect(wrapper.emitted("refresh")).toBeTruthy();
  });

  it("pause button is disabled when canPause is false", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canPause: false }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const pauseBtn = buttons.find((b) => b.text() === "inspector.pause")!;
    expect(pauseBtn.attributes("disabled")).toBeDefined();
  });

  it("pause button is enabled when canPause is true", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canPause: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const pauseBtn = buttons.find((b) => b.text() === "inspector.pause")!;
    expect(pauseBtn.attributes("disabled")).toBeUndefined();
  });

  it("emits pause when pause button is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canPause: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const pauseBtn = buttons.find((b) => b.text() === "inspector.pause")!;
    await pauseBtn.trigger("click");
    expect(wrapper.emitted("pause")).toBeTruthy();
  });

  it("resume button is disabled when canResume is false", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canResume: false }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const resumeBtn = buttons.find((b) => b.text() === "inspector.resume")!;
    expect(resumeBtn.attributes("disabled")).toBeDefined();
  });

  it("resume button is enabled when canResume is true", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canResume: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const resumeBtn = buttons.find((b) => b.text() === "inspector.resume")!;
    expect(resumeBtn.attributes("disabled")).toBeUndefined();
  });

  it("emits resume when resume button is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canResume: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const resumeBtn = buttons.find((b) => b.text() === "inspector.resume")!;
    await resumeBtn.trigger("click");
    expect(wrapper.emitted("resume")).toBeTruthy();
  });

  it("cancel button is disabled when canCancel is false", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canCancel: false }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const cancelBtn = buttons.find((b) => b.text() === "inspector.cancel")!;
    expect(cancelBtn.attributes("disabled")).toBeDefined();
  });

  it("emits cancel when cancel button is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canCancel: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const cancelBtn = buttons.find((b) => b.text() === "inspector.cancel")!;
    await cancelBtn.trigger("click");
    expect(wrapper.emitted("cancel")).toBeTruthy();
  });

  it("emits close when close button is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    // Last button should be the close button (no text content)
    const closeBtn = buttons[buttons.length - 1];
    await closeBtn.trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("shows refreshing label when isRefreshingStatus is true", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, isRefreshingStatus: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const refreshBtn = buttons.find((b) => b.text() === "common.refreshing");
    expect(refreshBtn).toBeDefined();
  });

  it("shows pausing label when actionName is 'Pause'", () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview, canPause: true, actionName: "Pause" }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const pauseBtn = buttons.find((b) => b.text() === "inspector.pausing");
    expect(pauseBtn).toBeDefined();
  });

  // ── Collapse ────────────────────────────────────────────────
  it("toggles collapse when header is clicked", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });

    // Initially not collapsed
    expect(wrapper.classes()).not.toContain("collapsed");
    expect(wrapper.find(".detail-panel__body").isVisible()).toBe(true);

    // Click header to collapse
    await wrapper.find(".detail-panel__header").trigger("click");
    expect(wrapper.classes()).toContain("collapsed");

    // Click header again to expand
    await wrapper.find(".detail-panel__header").trigger("click");
    expect(wrapper.classes()).not.toContain("collapsed");
  });

  it("shows arrow-up icon when collapsed", async () => {
    const overview = createOverview();
    const wrapper = mount(DetailPanel, {
      props: createProps({ selectedOverview: overview }),
      global: { stubs },
    });

    const getArrow = () => wrapper.find(".detail-panel__arrow");
    expect(getArrow().classes()).toContain("i-ri-arrow-down-line");

    await wrapper.find(".detail-panel__header").trigger("click");
    expect(getArrow().classes()).toContain("i-ri-arrow-up-line");
  });

  // ── No action buttons when no selection ─────────────────────
  it("does not render action buttons when no overview selected", () => {
    const wrapper = mount(DetailPanel, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".detail-panel__actions").exists()).toBe(false);
  });
});
