import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import BtTrackerTable from "../../components/limedl/BtTrackerTable.vue";
import type { BtTrackerInfo } from "../../types/download";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiEmptyState: {
    template: '<div class="ui-empty-state-stub">{{ title }}</div>',
    props: ["title", "icon"],
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createTracker(overrides: Partial<BtTrackerInfo> = {}): BtTrackerInfo {
  return {
    url: "udp://tracker.example.com:6969/announce",
    ...overrides,
  };
}

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    trackers: [createTracker()],
    ...overrides,
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("BtTrackerTable", () => {
  it("renders DataTable with single 'url' column", () => {
    const wrapper = mount(BtTrackerTable, {
      props: createProps(),
      global: { stubs },
    });

    const ths = wrapper.findAll("thead th");
    expect(ths).toHaveLength(1);
    expect(ths[0].text()).toBe("inspector.trackerTable.url");
  });

  it("renders tracker URLs", () => {
    const trackers = [
      createTracker({ url: "udp://tracker1.example.com:6969/announce" }),
      createTracker({ url: "https://tracker2.example.com/announce" }),
    ];
    const wrapper = mount(BtTrackerTable, {
      props: createProps({ trackers }),
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells).toHaveLength(2);
    expect(cells[0].text()).toBe("udp://tracker1.example.com:6969/announce");
    expect(cells[1].text()).toBe("https://tracker2.example.com/announce");
  });

  it("passes empty-title prop", () => {
    const wrapper = mount(BtTrackerTable, {
      props: createProps({ trackers: [] }),
      global: { stubs },
    });

    const emptyState = wrapper.find(".ui-empty-state-stub");
    expect(emptyState.exists()).toBe(true);
    expect(emptyState.text()).toBe("inspector.trackerTable.empty");
  });

  it("renders with empty trackers array", () => {
    const wrapper = mount(BtTrackerTable, {
      props: createProps({ trackers: [] }),
      global: { stubs },
    });

    // No table rendered
    expect(wrapper.find("tbody").exists()).toBe(false);
    // Empty state is shown
    expect(wrapper.find(".ui-empty-state-stub").exists()).toBe(true);
  });
});
