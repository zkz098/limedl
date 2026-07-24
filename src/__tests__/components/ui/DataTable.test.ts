import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import DataTable from "../../../components/ui/DataTable.vue";

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiEmptyState: {
    template: '<div class="ui-empty-state-stub">{{ title }}{{ icon ? "|" + icon : "" }}</div>',
    props: ["title", "icon"],
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createColumns() {
  return [
    { key: "name", label: "Name", width: "40%" },
    { key: "size", label: "Size", align: "right" as const, width: "30%" },
    { key: "status", label: "Status", align: "center" as const },
  ];
}

function createRows() {
  return [
    { name: "file1.txt", size: "1.2 MB", status: "Complete" },
    { name: "file2.txt", size: "3.5 MB", status: "Downloading" },
  ];
}

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    columns: createColumns(),
    rows: createRows(),
    ...overrides,
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("DataTable", () => {
  it("renders table with headers from columns", () => {
    const wrapper = mount(DataTable, {
      props: createProps(),
      global: { stubs },
    });

    const ths = wrapper.findAll("thead th");
    expect(ths).toHaveLength(3);
    expect(ths[0].text()).toBe("Name");
    expect(ths[1].text()).toBe("Size");
    expect(ths[2].text()).toBe("Status");
  });

  it("renders data rows with correct cell values", () => {
    const wrapper = mount(DataTable, {
      props: createProps(),
      global: { stubs },
    });

    const rows = wrapper.findAll("tbody tr");
    expect(rows).toHaveLength(2);

    const firstCells = rows[0].findAll("td");
    expect(firstCells[0].text()).toBe("file1.txt");
    expect(firstCells[1].text()).toBe("1.2 MB");
    expect(firstCells[2].text()).toBe("Complete");

    const secondCells = rows[1].findAll("td");
    expect(secondCells[0].text()).toBe("file2.txt");
    expect(secondCells[1].text()).toBe("3.5 MB");
    expect(secondCells[2].text()).toBe("Downloading");
  });

  it("applies column width style when specified", () => {
    const wrapper = mount(DataTable, {
      props: createProps(),
      global: { stubs },
    });

    const ths = wrapper.findAll("thead th");
    // First column has width "40%"
    expect(ths[0].attributes("style")).toContain("width: 40%");
    // Second column has width "30%"
    expect(ths[1].attributes("style")).toContain("width: 30%");
    // Third column has no width specified — no inline style
    expect(ths[2].attributes("style")).toBeUndefined();

    // Cell widths should also be applied
    const cells = wrapper.findAll("tbody tr td");
    expect(cells[0].attributes("style")).toContain("width: 40%");
    expect(cells[1].attributes("style")).toContain("width: 30%");
    expect(cells[2].attributes("style")).toBeUndefined();
  });

  it("applies alignment class when specified", () => {
    const wrapper = mount(DataTable, {
      props: createProps(),
      global: { stubs },
    });

    const ths = wrapper.findAll("thead th");
    // First column: no align — no alignment class
    expect(ths[0].classes()).not.toContain("data-table__th--right");
    expect(ths[0].classes()).not.toContain("data-table__th--center");
    // Second column: align="right"
    expect(ths[1].classes()).toContain("data-table__th--right");
    // Third column: align="center"
    expect(ths[2].classes()).toContain("data-table__th--center");

    // Cell alignment classes should also match
    const cells = wrapper.findAll("tbody tr td");
    expect(cells[0].classes()).not.toContain("data-table__cell--right");
    expect(cells[0].classes()).not.toContain("data-table__cell--center");
    expect(cells[1].classes()).toContain("data-table__cell--right");
    expect(cells[2].classes()).toContain("data-table__cell--center");
  });

  it("shows empty state when rows is empty array", () => {
    const wrapper = mount(DataTable, {
      props: createProps({ rows: [] }),
      global: { stubs },
    });

    expect(wrapper.find("table").exists()).toBe(false);
    expect(wrapper.find(".ui-empty-state-stub").exists()).toBe(true);
  });

  it("shows empty state with custom emptyTitle", () => {
    const wrapper = mount(DataTable, {
      props: createProps({ rows: [], emptyTitle: "No data available" }),
      global: { stubs },
    });

    const emptyState = wrapper.find(".ui-empty-state-stub");
    expect(emptyState.text()).toContain("No data available");
  });

  it("shows empty state with custom emptyIcon", () => {
    const wrapper = mount(DataTable, {
      props: createProps({ rows: [], emptyTitle: "Empty", emptyIcon: "i-ri-inbox-line" }),
      global: { stubs },
    });

    const emptyState = wrapper.find(".ui-empty-state-stub");
    expect(emptyState.text()).toContain("Empty");
    expect(emptyState.text()).toContain("|i-ri-inbox-line");
  });

  it("uses rowKey for :key binding when provided and value exists", () => {
    const rows = [
      { id: "row-1", name: "alpha.txt", size: "1 KB", status: "Done" },
      { id: "row-2", name: "beta.txt", size: "2 KB", status: "Done" },
    ];
    const wrapper = mount(DataTable, {
      props: { columns: createColumns(), rows, rowKey: "id" },
      global: { stubs },
    });

    const rowEls = wrapper.findAll("tbody tr");
    expect(rowEls).toHaveLength(2);
    // Key attribute is internal to Vue but we can verify rows render correctly
    expect(rowEls[0].text()).toContain("alpha.txt");
    expect(rowEls[1].text()).toContain("beta.txt");
  });

  it("falls back to rowIndex for :key when rowKey is missing", () => {
    const rows = [
      { name: "only.txt", size: "1 KB", status: "Done" },
    ];
    const wrapper = mount(DataTable, {
      props: { columns: createColumns(), rows },
      global: { stubs },
    });

    const rowEls = wrapper.findAll("tbody tr");
    expect(rowEls).toHaveLength(1);
    expect(rowEls[0].text()).toContain("only.txt");
  });

  it("shows empty string for missing cell values", () => {
    const rows = [
      { name: "partial.txt", size: "2 KB" }, // missing "status"
    ];
    const wrapper = mount(DataTable, {
      props: { columns: createColumns(), rows },
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells).toHaveLength(3);
    expect(cells[0].text()).toBe("partial.txt");
    expect(cells[1].text()).toBe("2 KB");
    // Missing value should show empty string
    expect(cells[2].text()).toBe("");
  });
});
