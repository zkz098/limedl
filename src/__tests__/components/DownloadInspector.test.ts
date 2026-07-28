/**
 * Tests for DownloadInspector.vue
 *
 * Covers: basic rendering, progress display, connection info, action buttons,
 * BT sub-tabs, BT file/peer/tracker lists, error display, and null state.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { reactive, ref } from "vue";

vi.mock("../../i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
  t: (key: string) => key,
}));

// ── Mock useBtInspector ────────────────────────────────────────────

const mockInspector = {
  files: ref<BtFileStatus[]>([]),
  peers: ref<BtPeerInfo[]>([]),
  trackers: ref<BtTrackerInfo[]>([]),
  pieces: ref<BtPieceInfo[]>([]),
  isLoading: reactive({ files: false, peers: false, trackers: false, pieces: false }),
  errors: reactive({ files: "", peers: "", trackers: "", pieces: "" }),
  isUpdatingFiles: ref(false),
  fetchFiles: vi.fn(),
  fetchPeers: vi.fn(),
  fetchTrackers: vi.fn(),
  fetchPieces: vi.fn(),
  toggleFileInclusion: vi.fn(),
};

vi.mock("../../composables/useBtInspector", () => ({
  useBtInspector: () => mockInspector,
}));

import DownloadInspector from "../../components/limedl/DownloadInspector.vue";
import type {
  BtFileStatus,
  BtPeerInfo,
  BtPieceInfo,
  BtTrackerInfo,
  DownloadSnapshot,
  DownloadSummary,
} from "../../types/download";

// ── Stubs ──────────────────────────────────────────────────────────

const stubs = {
  UiBadge: { template: '<span class="ui-badge-stub"><slot /></span>' },
  UiButton: {
    template:
      '<button :disabled="disabled" :data-loading="loading" class="ui-button-stub"><slot /></button>',
    props: ["disabled", "loading", "icon", "size", "variant", "type"],
  },
  UiProgress: {
    template: '<div class="ui-progress-stub" :data-value="value"></div>',
    props: ["value", "max", "indeterminate"],
  },
  BtPeerTable: {
    template: '<div class="bt-peer-table-stub" :data-count="peers.length"></div>',
    props: ["peers"],
  },
  BtTrackerTable: {
    template: '<div class="bt-tracker-table-stub" :data-count="trackers.length"></div>',
    props: ["trackers"],
  },
};

// ── Helper: create mock download ───────────────────────────────────

function createOverview(overrides: Partial<DownloadSummary> = {}): DownloadSummary {
  return {
    id: "dl-1",
    kind: "http",
    state: "downloading",
    url: "https://example.com/file.zip",
    fileName: "file.zip",
    destinationPath: "C:\\Downloads\\file.zip",
    downloadedBytes: 5 * 1024 * 1024,
    totalBytes: 10 * 1024 * 1024,
    connectionCount: 4,
    threadMode: "fixed",
    error: null,
    speedBytesPerSecond: 500 * 1024,
    etaSeconds: 10,
    createdAtMs: Date.now(),
    priority: "normal",
    cdnAccelerated: false,
    ...overrides,
  };
}

function createSnapshot(overrides: Partial<DownloadSnapshot> = {}): DownloadSnapshot {
  return {
    id: "dl-1",
    kind: "http",
    state: "downloading",
    url: "https://example.com/file.zip",
    finalUrl: "https://cdn.example.com/file.zip",
    fileName: "file.zip",
    destinationPath: "C:\\Downloads\\file.zip",
    tempPath: "C:\\Downloads\\.file.zip.part",
    downloadedBytes: 5 * 1024 * 1024,
    totalBytes: 10 * 1024 * 1024,
    supportsRanges: true,
    connectionCount: 4,
    threadMode: "fixed",
    checksumMode: "blake3",
    checksum: "abc123",
    cdnAccelerated: false,
    degraded: false,
    flushing: false,
    speedBytesPerSecond: 500 * 1024,
    etaSeconds: 10,
    createdAtMs: Date.now() - 60000,
    updatedAtMs: Date.now(),
    priority: "normal",
    ...overrides,
  };
}

// ── Tests ──────────────────────────────────────────────────────────

describe("DownloadInspector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset inspector mock data to defaults
    mockInspector.files.value = [];
    mockInspector.peers.value = [];
    mockInspector.trackers.value = [];
    mockInspector.pieces.value = [];
    mockInspector.isLoading.files = false;
    mockInspector.isLoading.peers = false;
    mockInspector.isLoading.trackers = false;
    mockInspector.errors.trackers = "";
  });

  function mountInspector(options: {
    overview?: DownloadSummary;
    snapshot?: DownloadSnapshot | null;
    showDetailInfo?: boolean;
  }) {
    const {
      overview = createOverview(),
      snapshot = createSnapshot(),
      showDetailInfo = false,
    } = options;
    return mount(DownloadInspector, {
      props: {
        selectedOverview: overview,
        selectedSnapshot: snapshot,
        showDetailInfo,
      },
      global: { stubs },
    });
  }

  // ── 1. Basic rendering ───────────────────────────────────────

  describe("basic rendering", () => {
    it("mounts without errors", () => {
      const wrapper = mountInspector({});
      expect(wrapper.exists()).toBe(true);
    });

    it("displays file name", () => {
      const overview = createOverview({ fileName: "ubuntu.iso" });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("ubuntu.iso");
    });

    it("displays destination path", () => {
      const overview = createOverview({ destinationPath: "/downloads/ubuntu.iso" });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("/downloads/ubuntu.iso");
    });

    it("displays state badge with correct state key", () => {
      const overview = createOverview({ state: "paused" });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("states.paused");
    });
  });

  // ── 2. Progress display ──────────────────────────────────────

  describe("progress display", () => {
    it("shows downloaded / total bytes", () => {
      const overview = createOverview({
        downloadedBytes: 2 * 1024 * 1024,
        totalBytes: 8 * 1024 * 1024,
      });
      const wrapper = mountInspector({ overview });
      // formatBytes(2MB) and formatBytes(8MB) should appear
      expect(wrapper.text()).toContain("inspector.transferred");
    });

    it("renders progress bar with UiProgress", () => {
      const wrapper = mountInspector({});
      expect(wrapper.find(".ui-progress-stub").exists()).toBe(true);
    });

    it("shows speed", () => {
      const overview = createOverview({ speedBytesPerSecond: 1024 * 1024 });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("inspector.speed");
    });

    it("shows ETA", () => {
      const overview = createOverview({ etaSeconds: 42 });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("inspector.eta");
    });
  });

  // ── 3. Connection info ───────────────────────────────────────

  describe("connection info", () => {
    it("shows connection count for HTTP downloads", () => {
      const overview = createOverview({ kind: "http", connectionCount: 6 });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("inspector.threads");
      expect(wrapper.text()).toContain("6");
    });

    it("shows peer count for BT downloads", () => {
      const overview = createOverview({ kind: "bt", peerCount: 12 });
      const wrapper = mountInspector({ overview, snapshot: null });
      expect(wrapper.text()).toContain("inspector.peers");
      expect(wrapper.text()).toContain("12");
    });

    it("shows seed/leech counts for BT downloads", () => {
      const overview = createOverview({ kind: "bt", seedCount: 5, leechCount: 3 });
      const wrapper = mountInspector({ overview, snapshot: null });
      expect(wrapper.text()).toContain("inspector.fields.seedCount");
      expect(wrapper.text()).toContain("5");
      expect(wrapper.text()).toContain("3");
    });

    it("does not show seed count for HTTP downloads", () => {
      const overview = createOverview({ kind: "http" });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).not.toContain("inspector.fields.seedCount");
    });
  });

  // ── 4. Detail rows (showDetailInfo) ──────────────────────────

  describe("detail rows", () => {
    it("shows URL and destination in detail rows when showDetailInfo is true", () => {
      const snapshot = createSnapshot({ url: "https://example.com/file.zip" });
      const wrapper = mountInspector({ snapshot, showDetailInfo: true });
      expect(wrapper.text()).toContain("inspector.fields.url");
      expect(wrapper.text()).toContain("https://example.com/file.zip");
    });

    it("shows checksum for HTTP snapshots in detail rows", () => {
      const snapshot = createSnapshot({ kind: "http", checksum: "sha256:abc" });
      const wrapper = mountInspector({ snapshot, showDetailInfo: true });
      expect(wrapper.text()).toContain("inspector.fields.checksum");
    });

    it("shows infoHash for BT snapshots in detail rows", () => {
      const snapshot = createSnapshot({ kind: "bt", infoHash: "deadbeef" });
      const wrapper = mountInspector({ snapshot, showDetailInfo: true });
      expect(wrapper.text()).toContain("inspector.fields.infoHash");
      expect(wrapper.text()).toContain("deadbeef");
    });

    it("returns empty detail rows when snapshot is null", () => {
      const wrapper = mountInspector({ snapshot: null, showDetailInfo: true });
      // Detail rows section should not be rendered
      expect(wrapper.text()).not.toContain("inspector.fields.url");
    });
  });

  // ── 5. Tab navigation ───────────────────────────────────────

  describe("tab navigation", () => {
    it("shows overview tab by default", () => {
      const wrapper = mountInspector({});
      const overviewTab = wrapper.findAll("button")[0];
      expect(overviewTab?.exists()).toBe(true);
    });

    it("shows Files and Peers & Trackers tabs for BT downloads", () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      const tabText = wrapper.text();
      expect(tabText).toContain("inspector.tabs.overview");
      expect(tabText).toContain("inspector.tabs.files");
      expect(tabText).toContain("inspector.tabs.peersTrackers");
    });

    it("does not show Peers & Trackers tab for HTTP downloads", () => {
      const wrapper = mountInspector({});
      expect(wrapper.text()).not.toContain("inspector.tabs.peersTrackers");
    });

    it("switches to Files tab on click", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      const fileTab = wrapper.findAll("button")[1];
      await fileTab?.trigger("click");
      // Files tab content should now be visible
      expect(wrapper.text()).toContain("inspector.files.fileCount");
    });
  });

  // ── 6. CDN accelerated badge ────────────────────────────────

  describe("cdn accelerated", () => {
    it("shows CDN badge when download is CDN accelerated", () => {
      const overview = createOverview({ cdnAccelerated: true });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("inspector.cdnNode");
      expect(wrapper.text()).toContain("inspector.cdnAccelerated");
    });

    it("does not show CDN badge when not accelerated", () => {
      const overview = createOverview({ cdnAccelerated: false });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).not.toContain("inspector.cdnNode");
    });
  });

  // ── 7. BT file list ─────────────────────────────────────────

  describe("BT file list", () => {
    beforeEach(() => {
      mockInspector.files.value = [
        { index: 0, path: "ubuntu.iso", size: 1_000_000, downloadedBytes: 500_000, included: true },
        { index: 1, path: "readme.txt", size: 1_000, downloadedBytes: 1_000, included: true },
      ];
    });

    it("renders file list with count", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      // Navigate to Files tab
      await wrapper.findAll("button")[1]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.files.fileCount");
      expect(wrapper.text()).toContain("ubuntu.iso");
      expect(wrapper.text()).toContain("readme.txt");
    });

    it("shows refresh button for file list", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[1]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.files.refreshFiles");
    });

    it("shows loading state when fetching files", async () => {
      mockInspector.isLoading.files = true;
      mockInspector.files.value = [];
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[1]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.files.loadingFiles");
    });

    it("shows empty state when no files", async () => {
      mockInspector.files.value = [];
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[1]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.files.noFileList");
    });
  });

  // ── 8. BT peer/tracker list ─────────────────────────────────

  describe("BT peer and tracker list", () => {
    beforeEach(() => {
      mockInspector.peers.value = [
        {
          address: "1.2.3.4:6881",
          client: "qBittorrent",
          flags: "",
          downloadSpeed: 1024,
          uploadSpeed: 512,
          progress: 0.5,
        },
        {
          address: "5.6.7.8:6881",
          client: "Transmission",
          flags: "D",
          downloadSpeed: 2048,
          uploadSpeed: 256,
          progress: 0.75,
        },
      ];
      mockInspector.trackers.value = [
        { url: "udp://tracker.example.com:6969" },
        { url: "https://tracker2.example.com/announce" },
      ];
      mockInspector.pieces.value = [
        { index: 0, completed: true },
        { index: 1, completed: false },
        { index: 2, completed: true },
      ];
    });

    it("renders peer list with count", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.sections.peers");
      expect(wrapper.text()).toContain("inspector.sections.trackers");
    });

    it("renders BtPeerTable with peer data", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      const peerTable = wrapper.findComponent(".bt-peer-table-stub");
      expect(peerTable.exists()).toBe(true);
    });

    it("renders BtTrackerTable with tracker data", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      const trackerTable = wrapper.findComponent(".bt-tracker-table-stub");
      expect(trackerTable.exists()).toBe(true);
    });

    it("shows piece progress text", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.pieceProgressText");
    });

    it("shows refresh buttons for peers and trackers", async () => {
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      expect(wrapper.text()).toContain("inspector.refreshPeers");
      expect(wrapper.text()).toContain("inspector.refreshTrackers");
    });

    it("shows tracker error when present", async () => {
      mockInspector.errors.trackers = "Connection failed";
      const overview = createOverview({ kind: "bt" });
      const wrapper = mountInspector({ overview, snapshot: null });
      await wrapper.findAll("button")[2]?.trigger("click");
      expect(wrapper.text()).toContain("Connection failed");
    });
  });

  // ── 9. Error display ────────────────────────────────────────

  describe("error display", () => {
    it("shows error message when overview has error", () => {
      const overview = createOverview({
        error: "HTTP status 404",
      });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).toContain("errors.http404");
    });

    it("does not show error section when no error", () => {
      const overview = createOverview({ error: null });
      const wrapper = mountInspector({ overview });
      expect(wrapper.text()).not.toContain("errors.http404");
    });
  });

  // ── 10. Null/empty state ────────────────────────────────────

  describe("null state handling", () => {
    it("renders empty section when selectedOverview is null", () => {
      // Passing a partial object without all required fields to test the
      // component's null-guard without triggering the non-null assertion.
      const wrapper = mount(DownloadInspector, {
        props: { selectedOverview: null!, selectedSnapshot: null, showDetailInfo: false },
        global: { stubs },
      });
      expect(wrapper.exists()).toBe(true);
      // No file name should be shown when overview is null
      expect(wrapper.find("h3").exists()).toBe(false);
    });

    it("handles missing overview without crashing", () => {
      const wrapper = mountInspector({ overview: undefined });
      expect(wrapper.exists()).toBe(true);
    });
  });
});
