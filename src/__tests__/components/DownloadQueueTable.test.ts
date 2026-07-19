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

function createMockDownload(overrides: Record<string, unknown> = {}): DownloadSummary {
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
    speedBytesPerSecond: 1024 * 100,
    etaSeconds: 5,
    error: undefined,
    cdnAccelerated: false,
    degraded: false,
    flushing: false,
    createdAtMs: 1000,
    ...overrides,
  } as DownloadSummary;
}

// ── Stubs ──

const stubs = {
  UiBadge: true,
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
});
