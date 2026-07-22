import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import DownloadQueueTable from "../../components/limedl/DownloadQueueTable.vue";
import type { DownloadSummary, ViewOptions, MultiSelectState } from "../../types/download";

// ── Mocks ──

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("../../composables/useFloatingClose", () => ({
  useFloatingClose: vi.fn(),
}));

// ── Fixtures ──

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    downloads: [] as DownloadSummary[],
    selectedId: null as string | null,
    taskActionName: "",
    isAutoRefreshing: false,
    viewOptions: {
      sortKey: "added_at" as const,
      sortDirection: "desc" as const,
      compactView: false,
      visibleColumns: ["file", "size", "status", "progress", "speed", "eta"],
    } as ViewOptions,
    multiSelect: {
      multiSelectMode: false,
      selectedIds: new Set<string>(),
      removedDownloadIds: [] as string[],
    } as MultiSelectState,
    ...overrides,
  };
}

function createMockDownload(overrides: Partial<DownloadSummary> = {}): DownloadSummary {
  return {
    id: "test-1",
    kind: "http",
    state: "downloading",
    fileName: "test.zip",
    url: "https://example.com/test.zip",
    destinationPath: "/tmp/test.zip",
    totalBytes: 1024 * 1024,
    downloadedBytes: 512 * 1024,
    connectionCount: 4,
    threadMode: "adaptive",
    requestedThreadCount: null,
    desiredThreadCount: null,
    allocatedThreadCount: null,
    adaptiveProfile: null,
    threadNote: null,
    speedBytesPerSecond: 1024 * 100,
    etaSeconds: 5,
    uploadedBytes: null,
    uploadSpeedBytesPerSecond: null,
    peerCount: null,
    uploadStatus: null,
    infoHash: null,
    error: null,
    cdnAccelerated: false,
    createdAtMs: 1000,
    ...overrides,
  };
}

// ── Stubs ──

const stubs = {
  UiBadge: {
    template: '<span class="ui-badge-stub"><slot /></span>',
  },
  UiButton: {
    template:
      '<button :disabled="disabled" :data-icon="icon || iconRight" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "icon", "iconRight", "size", "variant", "type"],
  },
  UiProgress: true,
  UiEmptyState: { template: '<div class="ui-empty-state"><slot name="default" /></div>' },
  Teleport: false,
};

// ── Tests ──

describe("DownloadQueueTable", () => {
  // ── Rendering ──────────────────────────────────────────────

  it("renders empty state when downloads array is empty", () => {
    const wrapper = mount(DownloadQueueTable, {
      props: createProps(),
      global: { stubs },
    });

    // UiEmptyState stub renders with class "ui-empty-state"
    expect(wrapper.find(".ui-empty-state").exists()).toBe(true);
  });

  it("renders rows for each download in downloads prop", () => {
    const downloads = [
      createMockDownload({ id: "1", fileName: "a.zip" }),
      createMockDownload({ id: "2", fileName: "b.zip" }),
      createMockDownload({ id: "3", fileName: "c.zip" }),
    ];
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    const rows = wrapper.findAll("tbody tr");
    expect(rows).toHaveLength(3);
  });

  it("renders file name in each row", () => {
    const downloads = [
      createMockDownload({ id: "1", fileName: "my-document.pdf" }),
      createMockDownload({ id: "2", fileName: "image.png" }),
    ];
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    const rows = wrapper.findAll("tbody tr");
    expect(rows[0].text()).toContain("my-document.pdf");
    expect(rows[1].text()).toContain("image.png");
  });

  it("renders progress bar for downloading tasks", () => {
    const downloads = [createMockDownload({ id: "1", state: "downloading" })];
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    const progressBars = wrapper.findAllComponents({ name: "UiProgress" });
    expect(progressBars.length).toBeGreaterThanOrEqual(1);
  });

  // ── Pagination ─────────────────────────────────────────────

  it("shows first page when more than pageSize downloads", () => {
    const downloads = Array.from({ length: 25 }, (_, i) =>
      createMockDownload({
        id: `${i + 1}`,
        fileName: `file-${i + 1}.zip`,
        createdAtMs: 1000 + i,
      }),
    );
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    // Page 1 should show 20 rows (pageSize)
    const rows = wrapper.findAll("tbody tr");
    expect(rows).toHaveLength(20);

    // Previous button should be disabled on page 1
    const buttons = wrapper.findAll("button.ui-button-stub");
    const prevButton = buttons.find((b) => b.text() === "queue.previous");
    expect(prevButton).toBeDefined();
    expect(prevButton!.attributes("disabled")).toBeDefined();
  });

  it("clicking next page shows page 2 content", async () => {
    const downloads = Array.from({ length: 25 }, (_, i) =>
      createMockDownload({
        id: `${i + 1}`,
        fileName: `file-${i + 1}.zip`,
        createdAtMs: 1000 + i,
      }),
    );
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    // Click the "next" button
    const buttons = wrapper.findAll("button.ui-button-stub");
    const nextButton = buttons.find((b) => b.text() === "queue.next");
    expect(nextButton).toBeDefined();
    await nextButton!.trigger("click");

    // Page 2 should show the remaining 5 rows
    const rows = wrapper.findAll("tbody tr");
    expect(rows).toHaveLength(5);

    // Previous button should now be enabled
    const updatedButtons = wrapper.findAll("button.ui-button-stub");
    const prevButton = updatedButtons.find((b) => b.text() === "queue.previous");
    expect(prevButton!.attributes("disabled")).toBeUndefined();
  });

  // ── Selection ──────────────────────────────────────────────

  it("clicking a row emits select event with download id", async () => {
    const downloads = [createMockDownload({ id: "select-1", fileName: "select-me.zip" })];
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    const row = wrapper.find("tbody tr");
    await row.trigger("click");

    expect(wrapper.emitted("select")).toBeTruthy();
    expect(wrapper.emitted("select")![0]).toEqual(["select-1"]);
  });

  it("selectedId prop highlights the correct row", () => {
    const downloads = [
      createMockDownload({ id: "active-1", fileName: "active.zip" }),
      createMockDownload({ id: "inactive-1", fileName: "inactive.zip" }),
    ];
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads, selectedId: "active-1" }),
      global: { stubs },
    });

    const rows = wrapper.findAll("tbody tr");

    // Active row should have the highlight class
    const activeRow = rows.find((r) => r.text().includes("active.zip"));
    expect(activeRow!.classes()).toContain("queue-row--active");

    // Inactive row should NOT have the highlight class
    const inactiveRow = rows.find((r) => r.text().includes("inactive.zip"));
    expect(inactiveRow!.classes()).not.toContain("queue-row--active");
  });

  // ── Sorting ────────────────────────────────────────────────

  it("changing sortKey prop reorders the rows", async () => {
    const downloads = [
      createMockDownload({ id: "a", fileName: "alpha.zip", createdAtMs: 300 }),
      createMockDownload({ id: "b", fileName: "beta.zip", createdAtMs: 100 }),
      createMockDownload({ id: "c", fileName: "gamma.zip", createdAtMs: 200 }),
    ];

    // Start with descending added_at (300, 200, 100)
    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      global: { stubs },
    });

    // Default sort is added_at desc → alpha (300), gamma (200), beta (100)
    let rows = wrapper.findAll("tbody tr td:first-child");
    expect(rows[0].text()).toContain("alpha.zip");
    expect(rows[1].text()).toContain("gamma.zip");
    expect(rows[2].text()).toContain("beta.zip");

    // Change to ascending name sort
    await wrapper.setProps({
      viewOptions: {
        sortKey: "name",
        sortDirection: "asc",
        compactView: false,
        visibleColumns: ["file", "size", "status", "progress", "speed", "eta"],
      },
    });

    rows = wrapper.findAll("tbody tr td:first-child");
    expect(rows[0].text()).toContain("alpha.zip");
    expect(rows[1].text()).toContain("beta.zip");
    expect(rows[2].text()).toContain("gamma.zip");
  });

  it("sort by name in ascending order", () => {
    const downloads = [
      createMockDownload({ id: "c", fileName: "c.zip", createdAtMs: 100 }),
      createMockDownload({ id: "a", fileName: "a.zip", createdAtMs: 200 }),
      createMockDownload({ id: "b", fileName: "b.zip", createdAtMs: 300 }),
    ];

    const wrapper = mount(DownloadQueueTable, {
      props: createProps({
        downloads,
        viewOptions: {
          sortKey: "name",
          sortDirection: "asc",
          compactView: false,
          visibleColumns: ["file", "size", "status", "progress", "speed", "eta"],
        },
      }),
      global: { stubs },
    });

    const rows = wrapper.findAll("tbody tr td:first-child");
    expect(rows[0].text()).toContain("a.zip");
    expect(rows[1].text()).toContain("b.zip");
    expect(rows[2].text()).toContain("c.zip");
  });

  // ── Context Menu ───────────────────────────────────────────

  it("right-click opens context menu at mouse position", async () => {
    const downloads = [createMockDownload({ id: "ctx-1", fileName: "context-test.zip" })];

    // Stub Teleport to render children inline (avoids jsdom Teleport issues)
    const inlineTeleportStubs = {
      ...stubs,
      Teleport: { template: "<div><slot /></div>" },
    };

    const wrapper = mount(DownloadQueueTable, {
      props: createProps({ downloads }),
      attachTo: document.body,
      global: { stubs: inlineTeleportStubs },
    });

    const row = wrapper.find("tbody tr");
    await row.trigger("contextmenu", {
      clientX: 200,
      clientY: 150,
    });

    // The context menu should now be visible
    // With inline teleport stub, it renders inside the wrapper
    const menu = wrapper.find(".task-context-menu");
    expect(menu.exists()).toBe(true);
    expect(menu.attributes("style")).toContain("left: 200px");
    expect(menu.attributes("style")).toContain("top: 150px");
  });

  // ── Multi-Select ───────────────────────────────────────────

  it("multi-select checkbox appears when multiSelect.multiSelectMode is true", () => {
    const downloads = [
      createMockDownload({ id: "ms-1", fileName: "multi-select.zip" }),
      createMockDownload({ id: "ms-2", fileName: "another.zip" }),
    ];

    const wrapper = mount(DownloadQueueTable, {
      props: createProps({
        downloads,
        multiSelect: {
          multiSelectMode: true,
          selectedIds: new Set<string>(),
          removedDownloadIds: [] as string[],
        },
      }),
      global: { stubs },
    });

    // Checkboxes should render in both the header and each row
    const checkboxes = wrapper.findAll('input[type="checkbox"]');
    expect(checkboxes.length).toBeGreaterThanOrEqual(3); // 1 header + 2 rows
  });

  it("clicking row in multiSelect mode emits toggleSelect", async () => {
    const downloads = [createMockDownload({ id: "ms-1", fileName: "multi-select.zip" })];

    const wrapper = mount(DownloadQueueTable, {
      props: createProps({
        downloads,
        multiSelect: {
          multiSelectMode: true,
          selectedIds: new Set<string>(),
          removedDownloadIds: [] as string[],
        },
      }),
      global: { stubs },
    });

    const row = wrapper.find("tbody tr");
    await row.trigger("click");

    expect(wrapper.emitted("toggleSelect")).toBeTruthy();
    expect(wrapper.emitted("toggleSelect")![0]).toEqual(["ms-1"]);
  });

  // ── Context Menu Actions ────────────────────────────────────

  describe("context menu actions", () => {
    const inlineTeleportStubs = {
      ...stubs,
      Teleport: { template: "<div><slot /></div>" },
    };

    it("pause/resume button emits pauseOrResume for downloading task", async () => {
      const downloads = [createMockDownload({ id: "pause-1", state: "downloading" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      // First .task-context-menu__item is the pause/resume button
      const pauseBtn = wrapper.find(".task-context-menu__item");
      expect(pauseBtn.exists()).toBe(true);
      expect(pauseBtn.attributes("disabled")).toBeUndefined();
      await pauseBtn.trigger("click");

      expect(wrapper.emitted("pauseOrResume")).toBeTruthy();
      expect(wrapper.emitted("pauseOrResume")![0]).toEqual(["pause-1"]);
    });

    it("pause/resume button is enabled for paused tasks and shows continue label", async () => {
      const downloads = [createMockDownload({ id: "pause-2", state: "paused" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const pauseBtn = wrapper.find(".task-context-menu__item");
      expect(pauseBtn.attributes("disabled")).toBeUndefined();
      expect(pauseBtn.text()).toContain("queue.continue");
    });

    it("pause/resume button is disabled for completed tasks", async () => {
      const downloads = [createMockDownload({ id: "pause-3", state: "completed" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const pauseBtn = wrapper.find(".task-context-menu__item");
      expect(pauseBtn.attributes("disabled")).toBeDefined();
    });

    it("delete button emits deleteTask", async () => {
      const downloads = [createMockDownload({ id: "del-1" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const deleteBtn = allButtons.find((b) => b.text().includes("queue.deleteTask"));
      expect(deleteBtn).toBeDefined();
      await deleteBtn!.trigger("click");

      expect(wrapper.emitted("deleteTask")).toBeTruthy();
      expect(wrapper.emitted("deleteTask")![0]).toEqual(["del-1"]);
    });

    it("copy link button emits copyLink", async () => {
      const downloads = [createMockDownload({ id: "copy-1" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const copyBtn = allButtons.find((b) => b.text().includes("queue.copyLink"));
      expect(copyBtn).toBeDefined();
      await copyBtn!.trigger("click");

      expect(wrapper.emitted("copyLink")).toBeTruthy();
      expect(wrapper.emitted("copyLink")![0]).toEqual(["copy-1"]);
    });

    it("open in explorer button emits openInExplorer", async () => {
      const downloads = [createMockDownload({ id: "open-1" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const openBtn = allButtons.find((b) => b.text().includes("queue.openInExplorer"));
      expect(openBtn).toBeDefined();
      await openBtn!.trigger("click");

      expect(wrapper.emitted("openInExplorer")).toBeTruthy();
      expect(wrapper.emitted("openInExplorer")![0]).toEqual(["open-1"]);
    });

    it("delete permanently button emits deleteTaskPermanently", async () => {
      const downloads = [createMockDownload({ id: "delperm-1" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const permBtn = allButtons.find((b) => b.text().includes("queue.permanentDelete"));
      expect(permBtn).toBeDefined();
      await permBtn!.trigger("click");

      expect(wrapper.emitted("deleteTaskPermanently")).toBeTruthy();
      expect(wrapper.emitted("deleteTaskPermanently")![0]).toEqual(["delperm-1"]);
    });

    it("BT speed limit option renders for BT tasks", async () => {
      const downloads = [createMockDownload({ id: "bt-1", kind: "bt" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const speedBtn = allButtons.find((b) => b.text().includes("queue.setSpeedLimit"));
      expect(speedBtn).toBeDefined();
    });

    it("BT speed limit option does NOT render for HTTP tasks", async () => {
      const downloads = [createMockDownload({ id: "http-1", kind: "http" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const allButtons = wrapper.findAll(".task-context-menu__item");
      const speedBtn = allButtons.find((b) => b.text().includes("queue.setSpeedLimit"));
      expect(speedBtn).toBeUndefined();
    });

    it("BT speed limit button emits setBtSpeedLimit", async () => {
      const downloads = [createMockDownload({ id: "bt-speed", kind: "bt" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const speedBtn = wrapper
        .findAll(".task-context-menu__item")
        .find((b) => b.text().includes("queue.setSpeedLimit"));
      await speedBtn!.trigger("click");

      expect(wrapper.emitted("setBtSpeedLimit")).toBeTruthy();
      expect(wrapper.emitted("setBtSpeedLimit")![0]).toEqual(["bt-speed"]);
    });

    it("context menu closes after clicking an action", async () => {
      const downloads = [createMockDownload({ id: "close-1" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      expect(wrapper.find(".task-context-menu").exists()).toBe(true);

      const deleteBtn = wrapper
        .findAll(".task-context-menu__item")
        .find((b) => b.text().includes("queue.deleteTask"));
      await deleteBtn!.trigger("click");

      // Menu should be closed after action
      expect(wrapper.find(".task-context-menu").exists()).toBe(false);
    });

    it("context menu also closes after permanent delete", async () => {
      const downloads = [createMockDownload({ id: "close-2" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        attachTo: document.body,
        global: { stubs: inlineTeleportStubs },
      });

      const row = wrapper.find("tbody tr");
      await row.trigger("contextmenu", { clientX: 100, clientY: 100 });

      const permBtn = wrapper
        .findAll(".task-context-menu__item")
        .find((b) => b.text().includes("queue.permanentDelete"));
      await permBtn!.trigger("click");

      expect(wrapper.find(".task-context-menu").exists()).toBe(false);
    });
  });

  // ── Meta Formatting ─────────────────────────────────────────

  describe("meta formatting", () => {
    it("HTTP download meta shows thread mode and connection count", () => {
      const downloads = [
        createMockDownload({
          id: "meta-http",
          kind: "http",
          threadMode: "adaptive",
          connectionCount: 4,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const meta = wrapper.find(".queue-file__meta");
      expect(meta.text()).toContain("queue.adaptive");
      expect(meta.text()).toContain("queue.currentThreads");
    });

    it("HTTP download meta shows fixed thread mode when not adaptive", () => {
      const downloads = [
        createMockDownload({
          id: "meta-fixed",
          kind: "http",
          threadMode: "fixed",
          connectionCount: 8,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const meta = wrapper.find(".queue-file__meta");
      expect(meta.text()).toContain("queue.fixedThread");
      expect(meta.text()).toContain("queue.currentThreads");
    });

    it("HTTP download meta includes adaptive profile and thread note when present", () => {
      const downloads = [
        createMockDownload({
          id: "meta-profile",
          kind: "http",
          threadMode: "adaptive",
          connectionCount: 4,
          adaptiveProfile: "aggressive",
          threadNote: "rate-limited",
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const meta = wrapper.find(".queue-file__meta");
      expect(meta.text()).toContain("tokens.aggressive");
      expect(meta.text()).toContain("rate-limited");
    });

    it("BT download meta shows upload status, seeds/leeches, peers, and uploaded bytes", () => {
      const downloads = [
        createMockDownload({
          id: "meta-bt",
          kind: "bt",
          uploadStatus: "uploading",
          seedCount: 5,
          leechCount: 3,
          peerCount: 10,
          uploadedBytes: 1024 * 50,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const meta = wrapper.find(".queue-file__meta");
      expect(meta.text()).toContain("uploadStatus.uploading");
      expect(meta.text()).toContain("5 S / 3 L");
      expect(meta.text()).toContain("queue.peerCount");
      expect(meta.text()).toContain("queue.uploaded");
    });

    it("BT download meta handles null seed/leech counts gracefully", () => {
      const downloads = [
        createMockDownload({
          id: "meta-bt-null",
          kind: "bt",
          uploadStatus: null,
          seedCount: null,
          leechCount: null,
          peerCount: 0,
          uploadedBytes: 0,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const meta = wrapper.find(".queue-file__meta");
      expect(meta.text()).not.toContain("S /");
      expect(meta.text()).toContain("uploadStatus.idle");
      expect(meta.text()).toContain("queue.uploaded");
    });
  });

  // ── Flushing State ──────────────────────────────────────────

  describe("flushing state", () => {
    it("flushing download shows 100% progress bar and 'Flushing' label", () => {
      const progressValueStubs = {
        ...stubs,
        UiProgress: {
          template: '<div class="ui-progress-stub" :data-value="value" />',
          props: ["value"],
        },
      };

      const downloads = [
        createMockDownload({
          id: "flush-1",
          state: "downloading",
          flushing: true,
          downloadedBytes: 500,
          totalBytes: 1000,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs: progressValueStubs },
      });

      expect(wrapper.text()).toContain("queue.flushing");

      const progressBar = wrapper.find(".ui-progress-stub");
      expect(progressBar.attributes("data-value")).toBe("100");
    });

    it("flushing download shows info-toned status badge with flushing short label", () => {
      const downloads = [
        createMockDownload({
          id: "flush-2",
          state: "downloading",
          flushing: true,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const statusCell = wrapper.find(".queue-cell--status");
      expect(statusCell.exists()).toBe(true);
      expect(statusCell.text()).toContain("queue.flushingShort");
    });

    it("non-flushing download uses normal progress label", () => {
      const downloads = [
        createMockDownload({
          id: "normal-1",
          state: "downloading",
          flushing: false,
          downloadedBytes: 256 * 1024,
          totalBytes: 1024 * 1024,
        }),
      ];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      expect(wrapper.text()).toContain("25.0%");
      expect(wrapper.text()).not.toContain("queue.flushing");
    });
  });

  // ── Auto-Refresh Indicator ──────────────────────────────────

  describe("auto-refresh indicator", () => {
    it("shows idle state when isAutoRefreshing is false", () => {
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ isAutoRefreshing: false }),
        global: { stubs },
      });

      const syncPill = wrapper.find(".sync-pill");
      expect(syncPill.exists()).toBe(true);
      expect(syncPill.attributes("data-active")).toBe("false");
    });

    it("shows syncing state when isAutoRefreshing becomes true after debounce", async () => {
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ isAutoRefreshing: false, downloads: [createMockDownload()] }),
        global: { stubs },
      });

      const syncPill = wrapper.find(".sync-pill");
      expect(syncPill.attributes("data-active")).toBe("false");

      await wrapper.setProps({ isAutoRefreshing: true });

      // Wait for the 240ms debounce show delay to elapse
      await new Promise((resolve) => setTimeout(resolve, 300));

      expect(syncPill.attributes("data-active")).toBe("true");
    });

    it("shows idle state when isAutoRefreshing returns to false after debounce", async () => {
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ isAutoRefreshing: true, downloads: [createMockDownload()] }),
        global: { stubs },
      });

      // Wait for the 240ms show debounce
      await new Promise((resolve) => setTimeout(resolve, 300));

      const syncPill = wrapper.find(".sync-pill");
      expect(syncPill.attributes("data-active")).toBe("true");

      await wrapper.setProps({ isAutoRefreshing: false });

      // Wait for the 420ms hide debounce
      await new Promise((resolve) => setTimeout(resolve, 500));

      expect(syncPill.attributes("data-active")).toBe("false");
    });
  });

  // ── Badge Rendering ─────────────────────────────────────────

  describe("badge rendering", () => {
    it("renders CDN badge when download is cdnAccelerated", () => {
      const downloads = [createMockDownload({ id: "cdn-1", cdnAccelerated: true })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const cdnBadge = wrapper.find(".queue-file__cdn");
      expect(cdnBadge.exists()).toBe(true);
    });

    it("does not render CDN badge when download is not cdnAccelerated", () => {
      const downloads = [createMockDownload({ id: "no-cdn-1", cdnAccelerated: false })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const cdnBadge = wrapper.find(".queue-file__cdn");
      expect(cdnBadge.exists()).toBe(false);
    });

    it("renders degraded badge when download is degraded", () => {
      const downloads = [createMockDownload({ id: "deg-1", degraded: true })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const degradedBadge = wrapper.find(".queue-file__degraded");
      expect(degradedBadge.exists()).toBe(true);
    });

    it("does not render degraded badge when download is not degraded", () => {
      const downloads = [createMockDownload({ id: "no-deg-1", degraded: false })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const degradedBadge = wrapper.find(".queue-file__degraded");
      expect(degradedBadge.exists()).toBe(false);
    });

    it("renders HDD badge when diskType is hdd", () => {
      const downloads = [createMockDownload({ id: "hdd-1", diskType: "hdd" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const hddBadge = wrapper.find(".queue-file__hdd");
      expect(hddBadge.exists()).toBe(true);
    });

    it("does not render HDD badge when diskType is ssd", () => {
      const downloads = [createMockDownload({ id: "ssd-1", diskType: "ssd" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      const hddBadge = wrapper.find(".queue-file__hdd");
      expect(hddBadge.exists()).toBe(false);
    });

    it("renders BT kind badge for BT downloads", () => {
      const downloads = [createMockDownload({ id: "bt-kind-1", kind: "bt" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      expect(wrapper.text()).toContain("tokens.bt");
    });

    it("renders HTTP kind badge for HTTP downloads", () => {
      const downloads = [createMockDownload({ id: "http-kind-1", kind: "http" })];
      const wrapper = mount(DownloadQueueTable, {
        props: createProps({ downloads }),
        global: { stubs },
      });

      expect(wrapper.text()).toContain("tokens.http");
    });
  });
});
