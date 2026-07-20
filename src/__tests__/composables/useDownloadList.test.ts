import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { ref } from "vue";

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

// Mock the i18n module so `t()` returns predictable keys
vi.mock("../../i18n", () => ({
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) {
      const serialized = JSON.stringify(options);
      return `${key} ${serialized}`;
    }
    return key;
  }),
}));

import { invoke } from "#invoke";
import {
  createMockInvoke,
  mockTauriCommand,
  mockTauriCommandValue,
  resetTauriMocks,
} from "../mocks/tauri-mock";
import { useDownloadList } from "../../composables/useDownloadList";
import { createMockDownloadTask } from "../fixtures/downloads";

// After vi.mock, invoke is a vi.fn() — use vi.mocked to access mock methods
const mockInvoke = vi.mocked(invoke);
import type { DownloadSnapshot, DownloadSummary } from "../../types/download";
import type { Ref } from "vue";

describe("useDownloadList", () => {
  let downloads: Ref<DownloadSummary[]>;
  let selectedId: Ref<string | null>;
  let selectedSnapshot: Ref<DownloadSnapshot | null>;
  let allowAutoSelect: Ref<boolean>;
  let isAutoRefreshing: Ref<boolean>;
  // Using 'any' for mock function variables to avoid
  // strict assignability issues with vi.fn() return types.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let ensureSelection: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let setMessage: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let setError: any;

  beforeEach(() => {
    resetTauriMocks();
    mockInvoke.mockImplementation(createMockInvoke());

    downloads = ref<DownloadSummary[]>([]);
    selectedId = ref<string | null>(null);
    selectedSnapshot = ref<DownloadSnapshot | null>(null);
    allowAutoSelect = ref(true);
    isAutoRefreshing = ref(false);
    ensureSelection = vi.fn();
    setMessage = vi.fn();
    setError = vi.fn();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  function createList() {
    return useDownloadList({
      downloads,
      selectedId,
      selectedSnapshot,
      allowAutoSelect,
      isAutoRefreshing,
      ensureSelection,
      setMessage,
      setError,
    });
  }

  // ── Initial state ──────────────────────────────────────────────────

  it("returns initial state correctly", () => {
    const list = createList();

    expect(list.downloads.value).toEqual([]);
    expect(list.isRefreshingList.value).toBe(false);
    expect(list.isAutoRefreshing.value).toBe(false);
  });

  // ── refreshList basic flow ─────────────────────────────────────────

  it("fetches downloads and populates the ref", async () => {
    const mockData = createMockDownloadTask({ id: "task-1", fileName: "test.zip" });
    mockTauriCommandValue("download_list", [mockData]);

    const list = createList();
    await list.refreshList();

    expect(list.downloads.value).toHaveLength(1);
    expect(list.downloads.value[0].id).toBe("task-1");
    expect(list.downloads.value[0].fileName).toBe("test.zip");
  });

  it("calls listDownloads via Tauri invoke", async () => {
    const mockData = createMockDownloadTask();
    mockTauriCommandValue("download_list", [mockData]);

    const list = createList();
    await list.refreshList();

    expect(mockInvoke).toHaveBeenCalledWith("download_list");
  });

  it("calls ensureSelection after fetching downloads", async () => {
    const mockData = createMockDownloadTask();
    mockTauriCommandValue("download_list", [mockData]);

    const list = createList();
    await list.refreshList();

    expect(ensureSelection).toHaveBeenCalledTimes(1);
  });

  it("sets a message when downloads are fetched successfully", async () => {
    const mockData = createMockDownloadTask();
    mockTauriCommandValue("download_list", [mockData]);

    const list = createList();
    await list.refreshList();

    expect(setMessage).toHaveBeenCalledWith(expect.stringContaining("messages.queueRefreshed"));
  });

  it("sets a 'no downloads' message when list is empty", async () => {
    mockTauriCommandValue("download_list", []);

    const list = createList();
    await list.refreshList();

    expect(setMessage).toHaveBeenCalledWith("messages.noDownloads");
  });

  // ── Silent mode ────────────────────────────────────────────────────

  it("does not set messages in silent mode", async () => {
    const mockData = createMockDownloadTask();
    mockTauriCommandValue("download_list", [mockData]);

    const list = createList();
    await list.refreshList({ silent: true });

    expect(setMessage).not.toHaveBeenCalled();
    expect(setError).not.toHaveBeenCalled();
  });

  it("does not set 'no downloads' message in silent mode", async () => {
    mockTauriCommandValue("download_list", []);

    const list = createList();
    await list.refreshList({ silent: true });

    expect(setMessage).not.toHaveBeenCalled();
  });

  // ── Error handling ─────────────────────────────────────────────────

  it("calls setError when listDownloads throws", async () => {
    mockTauriCommand("download_list", () => {
      throw new Error("Backend unavailable");
    });

    const list = createList();
    await list.refreshList();

    expect(setError).toHaveBeenCalledWith("Backend unavailable");
    expect(setMessage).not.toHaveBeenCalled();
  });

  it("does not call setError in silent mode when fetch fails", async () => {
    mockTauriCommand("download_list", () => {
      throw new Error("Backend unavailable");
    });

    const list = createList();
    await list.refreshList({ silent: true });

    expect(setError).not.toHaveBeenCalled();
  });

  // ── Concurrent refresh guard ───────────────────────────────────────

  it("guards against concurrent refreshes", async () => {
    const mockData = createMockDownloadTask();
    let resolvePromise: (value: DownloadSummary[]) => void;
    const pendingPromise = new Promise<DownloadSummary[]>((resolve) => {
      resolvePromise = resolve;
    });

    mockTauriCommandValue("download_list", () => pendingPromise);

    const list = createList();
    const refresh1 = list.refreshList();
    const refresh2 = list.refreshList(); // should be a no-op

    resolvePromise!([mockData]);
    await refresh1;
    await refresh2;

    // invoke should only have been called once
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });

  // ── state management ───────────────────────────────────────────────

  it("resets selectedSnapshot when selectedId changes", async () => {
    const mockData = createMockDownloadTask({ id: "task-1" });
    mockTauriCommandValue("download_list", [mockData]);

    // Set up a stale snapshot
    selectedId.value = "task-1";
    selectedSnapshot.value = {
      id: "old-snapshot-id",
      kind: "http",
      state: "downloading",
      url: "https://example.com/old.zip",
      finalUrl: "https://example.com/old.zip",
      fileName: "old.zip",
      destinationPath: "C:\\Downloads",
      tempPath: "C:\\Downloads\\.old.zip.part",
      supportsRanges: true,
      connectionCount: 0,
      threadMode: "fixed",
      checksumMode: "blake3",
      cdnAccelerated: false,
      degraded: false,
      flushing: false,
      downloadedBytes: 0,
      createdAtMs: Date.now(),
      updatedAtMs: Date.now(),
    } as DownloadSnapshot;

    // ensureSelection will set selectedId to "task-1" from the fresh data,
    // but it's a mock - so it does nothing. Let's set it up so the condition
    // selectedSnapshot.value.id !== selectedId.value is met
    ensureSelection.mockImplementation(() => {
      selectedId.value = "task-1";
    });

    const list = createList();
    await list.refreshList();

    // ensureSelection runs after downloads update, setting selectedId to task-1
    expect(ensureSelection).toHaveBeenCalled();

    // The snapshot's id (old-snapshot-id) !== selectedId (task-1), so it should be nulled
    expect(selectedSnapshot.value).toBeNull();
  });

  // ── Guard: refresh skipped while already refreshing ────────────────

  it("sets isRefreshingList to true during refresh and false after", async () => {
    let resolvePromise: (value: DownloadSummary[]) => void;
    const pendingPromise = new Promise<DownloadSummary[]>((resolve) => {
      resolvePromise = resolve;
    });

    mockTauriCommandValue("download_list", () => pendingPromise);

    const list = createList();
    const promise = list.refreshList();

    // During the refresh
    expect(list.isRefreshingList.value).toBe(true);

    resolvePromise!([]);
    await promise;

    // After refresh
    expect(list.isRefreshingList.value).toBe(false);
  });

  // ── Multiple items ─────────────────────────────────────────────────

  it("handles multiple download items", async () => {
    const mockData = [
      createMockDownloadTask({ id: "task-1", fileName: "a.zip" }),
      createMockDownloadTask({ id: "task-2", fileName: "b.zip" }),
      createMockDownloadTask({ id: "task-3", fileName: "c.zip" }),
    ];
    mockTauriCommandValue("download_list", mockData);

    const list = createList();
    await list.refreshList();

    expect(list.downloads.value).toHaveLength(3);
    expect(list.downloads.value.map((d) => d.fileName)).toEqual(["a.zip", "b.zip", "c.zip"]);
  });

  // ── Error message serialization ────────────────────────────────────

  it("converts non-Error throws to string messages", async () => {
    mockTauriCommand("download_list", () => {
      // eslint-disable-next-line no-throw-literal
      throw "String error";
    });

    const list = createList();
    await list.refreshList();

    expect(setError).toHaveBeenCalledWith("String error");
  });
});
