import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import TopToolbar from "../../components/layout/TopToolbar.vue";
import type { SortKey } from "../../types/settings";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../i18n", () => ({
  t: (key: string, options?: Record<string, unknown>) => {
    if (options && options.count !== undefined) {
      return `${key} count=${JSON.stringify(options.count)}`;
    }
    return key;
  },
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("../../lib/download-format", () => ({
  formatSpeed: (value: number) => `${value} B/s`,
}));

vi.mock("../../composables/useFloatingClose", () => ({
  useFloatingClose: vi.fn(),
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiButton: {
    template:
      '<button :disabled="disabled" :data-icon="icon || iconRight" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "icon", "iconRight", "size", "variant", "type"],
  },
  UiSelect: {
    template:
      '<select class="ui-select-stub" :value="modelValue" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="opt in options" :key="opt.value" :value="opt.value">{{ opt.label }}</option></select>',
    props: ["modelValue", "options", "disabled", "placeholder", "ariaLabel"],
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    searchQuery: "",
    hasSelection: false,
    btStatus: null,
    sortKey: "added_at" as SortKey,
    sortDirection: "desc" as const,
    compactView: false,
    visibleColumns: ["file", "size", "status", "progress", "speed", "eta"],
    multiSelectMode: false,
    selectedCount: 0,
    filteredCount: 0,
    ...overrides,
  };
}

describe("TopToolbar", () => {
  // ── Rendering ──────────────────────────────────────────────
  it("renders add task button", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const addBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.addTask");
    expect(addBtn).toBeDefined();
  });

  it("renders delete and refresh buttons", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("toolbar.delete");
    expect(wrapper.text()).toContain("toolbar.refresh");
  });

  it("delete button is disabled when hasSelection is false", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ hasSelection: false }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const deleteBtn = buttons.find((b) => b.text() === "toolbar.delete")!;
    expect(deleteBtn.attributes("disabled")).toBeDefined();
  });

  it("delete button is enabled when hasSelection is true", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ hasSelection: true }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const deleteBtn = buttons.find((b) => b.text() === "toolbar.delete")!;
    expect(deleteBtn.attributes("disabled")).toBeUndefined();
  });

  // ── Events ─────────────────────────────────────────────────
  it("emits add-task when add task button is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    await wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.addTask")!
      .trigger("click");
    expect(wrapper.emitted("add-task")).toBeTruthy();
  });

  it("emits delete when delete button is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ hasSelection: true }),
      global: { stubs },
    });
    const deleteBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.delete")!;
    await deleteBtn.trigger("click");
    expect(wrapper.emitted("delete")).toBeTruthy();
  });

  it("emits refresh when refresh button is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const refreshBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.refresh")!;
    await refreshBtn.trigger("click");
    expect(wrapper.emitted("refresh")).toBeTruthy();
  });

  // ── Multi-select mode ──────────────────────────────────────
  it("renders multi-select toggle button", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const msBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.multiSelectMode")!;
    expect(msBtn).toBeDefined();
  });

  it("emits update:multiSelectMode when toggle clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: false }),
      global: { stubs },
    });
    const msBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.multiSelectMode")!;
    await msBtn.trigger("click");
    expect(wrapper.emitted("update:multiSelectMode")).toBeTruthy();
    expect(wrapper.emitted("update:multiSelectMode")![0]).toEqual([true]);
  });

  it("shows batch action buttons when multiSelectMode is true", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 3, filteredCount: 10 }),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("toolbar.pauseAll");
    expect(wrapper.text()).toContain("toolbar.resumeAll");
    expect(wrapper.text()).toContain("toolbar.clearCompleted");
    expect(wrapper.text()).toContain("toolbar.selectAll");
    expect(wrapper.text()).toContain("toolbar.batchDelete");
    expect(wrapper.text()).toContain("toolbar.selectedCount count=3");
  });

  it("hides batch action buttons when multiSelectMode is false", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: false }),
      global: { stubs },
    });
    expect(wrapper.text()).not.toContain("toolbar.pauseAll");
    expect(wrapper.text()).not.toContain("toolbar.batchDelete");
    expect(wrapper.find(".toolbar-batch-actions").exists()).toBe(false);
  });

  it("batch delete is disabled when selectedCount is 0", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 0 }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const batchDeleteBtn = buttons.find((b) => b.text() === "toolbar.batchDelete")!;
    expect(batchDeleteBtn.attributes("disabled")).toBeDefined();
  });

  it("batch delete is enabled when selectedCount > 0", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 3 }),
      global: { stubs },
    });
    const buttons = wrapper.findAll("button.ui-button-stub");
    const batchDeleteBtn = buttons.find((b) => b.text() === "toolbar.batchDelete")!;
    expect(batchDeleteBtn.attributes("disabled")).toBeUndefined();
  });

  it("emits batchDelete when batch delete is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 2 }),
      global: { stubs },
    });
    const batchDeleteBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.batchDelete")!;
    await batchDeleteBtn.trigger("click");
    expect(wrapper.emitted("batchDelete")).toBeTruthy();
  });

  it("emits pauseAll when pause all is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 1 }),
      global: { stubs },
    });
    const pauseAllBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.pauseAll")!;
    await pauseAllBtn.trigger("click");
    expect(wrapper.emitted("pauseAll")).toBeTruthy();
  });

  it("emits resumeAll when resume all is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 1 }),
      global: { stubs },
    });
    const resumeAllBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.resumeAll")!;
    await resumeAllBtn.trigger("click");
    expect(wrapper.emitted("resumeAll")).toBeTruthy();
  });

  it("emits clearCompleted when clear completed is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 1 }),
      global: { stubs },
    });
    const clearBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.clearCompleted")!;
    await clearBtn.trigger("click");
    expect(wrapper.emitted("clearCompleted")).toBeTruthy();
  });

  // ── Select all / deselect all ───────────────────────────────
  it("shows deselectAll when all items are selected", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 10, filteredCount: 10 }),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("toolbar.deselectAll");
    expect(wrapper.text()).not.toContain("toolbar.selectAll");
  });

  it("shows selectAll when not all items are selected", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 3, filteredCount: 10 }),
      global: { stubs },
    });
    expect(wrapper.text()).toContain("toolbar.selectAll");
    expect(wrapper.text()).not.toContain("toolbar.deselectAll");
  });

  it("emits deselectAll when select toggle clicked and all selected", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 10, filteredCount: 10 }),
      global: { stubs },
    });
    const toggleBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.deselectAll")!;
    await toggleBtn.trigger("click");
    expect(wrapper.emitted("deselectAll")).toBeTruthy();
  });

  it("emits selectAll when select toggle clicked and not all selected", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ multiSelectMode: true, selectedCount: 0, filteredCount: 10 }),
      global: { stubs },
    });
    const toggleBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.selectAll")!;
    await toggleBtn.trigger("click");
    expect(wrapper.emitted("selectAll")).toBeTruthy();
  });

  // ── Search ──────────────────────────────────────────────────
  it("renders search input", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "" }),
      global: { stubs },
    });
    expect(wrapper.find("input[type='text']").exists()).toBe(true);
  });

  it("search input shows current query value", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "test query" }),
      global: { stubs },
    });
    const input = wrapper.find("input");
    expect(input.attributes("value")).toBe("test query");
  });

  it("emits update:searchQuery on search input", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "" }),
      global: { stubs },
    });
    const input = wrapper.find("input");
    await input.setValue("new search");
    expect(wrapper.emitted("update:searchQuery")).toBeTruthy();
    expect(wrapper.emitted("update:searchQuery")![0]).toEqual(["new search"]);
  });

  it("shows clear button when search query is non-empty", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "text" }),
      global: { stubs },
    });
    expect(wrapper.find(".toolbar-search__clear").exists()).toBe(true);
  });

  it("hides clear button when search query is empty", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "" }),
      global: { stubs },
    });
    expect(wrapper.find(".toolbar-search__clear").exists()).toBe(false);
  });

  it("emits update:searchQuery with empty string on clear", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ searchQuery: "text" }),
      global: { stubs },
    });
    await wrapper.find(".toolbar-search__clear").trigger("click");
    expect(wrapper.emitted("update:searchQuery")![0]).toEqual([""]);
  });

  // ── Sort controls ───────────────────────────────────────────
  it("renders sort select and sort direction button", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".sort-control__select").exists()).toBe(true);
    expect(wrapper.find(".sort-control").exists()).toBe(true);
  });

  it("emits update:sortKey when sort select changes", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const select = wrapper.find(".ui-select-stub");
    await select.setValue("name");
    expect(wrapper.emitted("update:sortKey")).toBeTruthy();
    expect(wrapper.emitted("update:sortKey")![0]).toEqual(["name"]);
  });

  it("emits update:sortDirection when direction button clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ sortDirection: "asc" }),
      global: { stubs },
    });
    const sortControl = wrapper.find(".sort-control");
    const dirBtn = sortControl.find("button.ui-button-stub");
    await dirBtn.trigger("click");
    expect(wrapper.emitted("update:sortDirection")![0]).toEqual(["desc"]);
  });

  // ── Column menu ─────────────────────────────────────────────
  it("shows column menu panel when column button is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const colBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.columns")!;
    expect(wrapper.find(".column-menu__panel").exists()).toBe(false);

    await colBtn.trigger("click");
    expect(wrapper.find(".column-menu__panel").exists()).toBe(true);
  });

  it("renders column options with checkboxes in column menu", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    const colBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.columns")!;
    await colBtn.trigger("click");

    const items = wrapper.findAll(".column-menu__item");
    expect(items.length).toBeGreaterThanOrEqual(7); // VALID_COLUMN_KEYS length
  });

  it("emits update:visibleColumns when column checkbox is toggled", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({
        visibleColumns: ["file", "size", "status", "progress", "speed", "eta"],
      }),
      global: { stubs },
    });
    const colBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.text() === "toolbar.columns")!;
    await colBtn.trigger("click");

    // Toggle a visible column (uncheck "size")
    const checkboxes = wrapper.findAll('.column-menu__item input[type="checkbox"]');
    // Second checkbox should be "size"
    await checkboxes[1].setValue(false);
    expect(wrapper.emitted("update:visibleColumns")).toBeTruthy();
  });

  // ── Compact view ────────────────────────────────────────────
  it("emits update:compactView when compact button is clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ compactView: false }),
      global: { stubs },
    });
    const compactBtn = wrapper
      .findAll("button.ui-button-stub")
      .find((b) => b.attributes("data-icon") === "i-ri-list-check")!;
    await compactBtn.trigger("click");
    expect(wrapper.emitted("update:compactView")![0]).toEqual([true]);
  });

  // ── BT status ───────────────────────────────────────────────
  it("does not render BT status when btStatus is null", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ btStatus: null }),
      global: { stubs },
    });
    expect(wrapper.find('[data-testid="toolbar-bt-status"]').exists()).toBe(false);
  });

  it("renders BT status pills when btStatus is provided", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({
        btStatus: { dhtNodes: 42, uploadSpeed: 102400, peers: 7, torrents: 3 },
      }),
      global: { stubs },
    });
    expect(wrapper.find('[data-testid="toolbar-bt-status"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="toolbar-bt-dht-count"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="toolbar-bt-upload-speed"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("42");
    expect(wrapper.text()).toContain("102400 B/s");
    expect(wrapper.text()).toContain("7");
  });

  // ── Game mode ───────────────────────────────────────────────
  it("renders game mode button", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".game-mode-btn").exists()).toBe(true);
  });

  it("emits toggleGameMode when game mode button clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    await wrapper.find(".game-mode-btn").trigger("click");
    expect(wrapper.emitted("toggleGameMode")).toBeTruthy();
  });

  // ── Overclock mode ──────────────────────────────────────────
  it("renders overclock button", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    expect(wrapper.find(".overclock-btn").exists()).toBe(true);
  });

  it("emits toggleOverclockMode when overclock button clicked", async () => {
    const wrapper = mount(TopToolbar, {
      props: createProps(),
      global: { stubs },
    });
    await wrapper.find(".overclock-btn").trigger("click");
    expect(wrapper.emitted("toggleOverclockMode")).toBeTruthy();
  });

  it("applies active class when overclockMode is true", () => {
    const wrapper = mount(TopToolbar, {
      props: createProps({ overclockMode: true }),
      global: { stubs },
    });
    expect(wrapper.find(".overclock-btn--active").exists()).toBe(true);
  });
});
