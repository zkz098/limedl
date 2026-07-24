import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import BtPeerTable from "../../components/limedl/BtPeerTable.vue";
import type { BtPeerInfo } from "../../types/download";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("../../lib/download-format", () => ({
  formatSpeed: vi.fn((b: number) => `${b}B/s`),
}));

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiEmptyState: {
    template: '<div class="ui-empty-state-stub">{{ title }}</div>',
    props: ["title", "icon"],
  },
};

// ── Fixtures ───────────────────────────────────────────────────────

function createPeer(overrides: Partial<BtPeerInfo> = {}): BtPeerInfo {
  return {
    address: "192.168.1.1:6881",
    client: "qBittorrent 4.6.0",
    flags: "X",
    downloadSpeed: 1_048_576,
    uploadSpeed: 524_288,
    progress: 0.75,
    ...overrides,
  };
}

function createProps(overrides: Record<string, unknown> = {}) {
  return {
    peers: [createPeer()],
    ...overrides,
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("BtPeerTable", () => {
  it("renders DataTable with correct columns (6 columns)", () => {
    const wrapper = mount(BtPeerTable, {
      props: createProps(),
      global: { stubs },
    });

    const ths = wrapper.findAll("thead th");
    expect(ths).toHaveLength(6);
    expect(ths[0].text()).toBe("inspector.peerTable.ip");
    expect(ths[1].text()).toBe("inspector.peerTable.client");
    expect(ths[2].text()).toBe("inspector.peerTable.flags");
    expect(ths[3].text()).toBe("inspector.peerTable.dlSpeed");
    expect(ths[4].text()).toBe("inspector.peerTable.ulSpeed");
    expect(ths[5].text()).toBe("inspector.peerTable.progress");
  });

  it("renders peer data correctly mapped to rows", () => {
    const peer = createPeer({
      address: "10.0.0.1:51413",
      client: "Transmission 4.0",
      flags: "D",
      downloadSpeed: 2_097_152,
      uploadSpeed: 1_048_576,
      progress: 0.5,
    });
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [peer] }),
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells).toHaveLength(6);
    expect(cells[0].text()).toBe("10.0.0.1:51413");
    expect(cells[1].text()).toBe("Transmission 4.0");
    expect(cells[2].text()).toBe("D");
    expect(cells[3].text()).toBe("2097152B/s");
    expect(cells[4].text()).toBe("1048576B/s");
    expect(cells[5].text()).toBe("50%");
  });

  it('shows "—" for null client and flags', () => {
    const peer = createPeer({ client: "", flags: "" });
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [peer] }),
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells[1].text()).toBe("—");
    expect(cells[2].text()).toBe("—");
  });

  it("shows formatted speed for dlSpeed and ulSpeed", () => {
    const peer = createPeer({
      downloadSpeed: 5_000_000,
      uploadSpeed: 2_500_000,
    });
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [peer] }),
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells[3].text()).toBe("5000000B/s");
    expect(cells[4].text()).toBe("2500000B/s");
  });

  it("shows formatted progress percentage", () => {
    const peer = createPeer({ progress: 0.3333 });
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [peer] }),
      global: { stubs },
    });

    const cells = wrapper.findAll("tbody tr td");
    expect(cells[5].text()).toBe("33%");
  });

  it("passes empty-title prop to DataTable", () => {
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [] }),
      global: { stubs },
    });

    // When rows are empty, DataTable shows UiEmptyState with empty-title
    const emptyState = wrapper.find(".ui-empty-state-stub");
    expect(emptyState.exists()).toBe(true);
    expect(emptyState.text()).toBe("inspector.peerTable.empty");
  });

  it("renders with empty peers array (shows empty state)", () => {
    const wrapper = mount(BtPeerTable, {
      props: createProps({ peers: [] }),
      global: { stubs },
    });

    // No table rows rendered
    expect(wrapper.find("tbody").exists()).toBe(false);

    // UiEmptyState is rendered
    expect(wrapper.find(".ui-empty-state-stub").exists()).toBe(true);
  });
});
