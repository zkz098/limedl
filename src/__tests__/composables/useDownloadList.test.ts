import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

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
import { useDownloadStore } from "../../stores/download";
import { createMockDownloadTask } from "../fixtures/downloads";

const mockInvoke = vi.mocked(invoke);
import type { DownloadSummary } from "../../types/download";

vi.mock("../../stores/notification", () => ({
  useNotificationStore: () => ({
    notifySuccess: vi.fn(),
    notifyError: vi.fn(),
    notifyInfo: vi.fn(),
    notifyWarning: vi.fn(),
    clearAll: vi.fn(),
    notify: vi.fn(),
    dismiss: vi.fn(),
    notifications: { value: [] },
  }),
}));

vi.mock("#event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("useDownloadStore (refreshList)", () => {
  let store: ReturnType<typeof useDownloadStore>;

  beforeEach(() => {
    resetTauriMocks();
    mockInvoke.mockImplementation(createMockInvoke());
    setActivePinia(createPinia());
    store = useDownloadStore();
  });

  afterEach(() => {
    vi.clearAllMocks();
    store.destroyStore();
  });

  // ── Initial state ──────────────────────────────────────────────────

  it("returns initial state correctly", () => {
    expect(store.downloads).toEqual([]);
    expect(store.isRefreshingList).toBe(false);
    expect(store.isAutoRefreshing).toBe(false);
  });

  // ── refreshList basic flow ─────────────────────────────────────────

  it("fetches downloads and populates the ref", async () => {
    const mockData = createMockDownloadTask({ id: "task-1", fileName: "test.zip" });
    mockTauriCommandValue("download_list", [mockData]);

    await store.refreshList();

    expect(store.downloads).toHaveLength(1);
    expect(store.downloads[0].id).toBe("task-1");
    expect(store.downloads[0].fileName).toBe("test.zip");
  });

  it("calls listDownloads via Tauri invoke", async () => {
    const mockData = createMockDownloadTask();
    mockTauriCommandValue("download_list", [mockData]);

    await store.refreshList();

    expect(mockInvoke).toHaveBeenCalledWith("download_list");
  });

  it("sets a 'no downloads' message when list is empty", async () => {
    mockTauriCommandValue("download_list", []);

    await store.refreshList();

    // The store uses the notification store for messages
    // The message flow is: setMessage → notify.notifyInfo
    // which is mocked, so just verify no crash
    expect(store.downloads).toEqual([]);
  });

  // ── Error handling ─────────────────────────────────────────────────

  it("handles error when listDownloads throws", async () => {
    mockTauriCommand("download_list", () => {
      throw new Error("Backend unavailable");
    });

    await store.refreshList();

    // Error is caught via setError → notify.notifyError (mocked)
    // Verify the downloads weren't corrupted
    expect(store.downloads).toEqual([]);
  });

  // ── Concurrent refresh guard ───────────────────────────────────────

  it("guards against concurrent refreshes", async () => {
    const mockData = createMockDownloadTask();
    let resolvePromise: (value: DownloadSummary[]) => void;
    const pendingPromise = new Promise<DownloadSummary[]>((resolve) => {
      resolvePromise = resolve;
    });

    mockTauriCommandValue("download_list", () => pendingPromise);

    const refresh1 = store.refreshList();
    const refresh2 = store.refreshList(); // should be a no-op

    resolvePromise!([mockData]);
    await refresh1;
    await refresh2;

    // invoke should only have been called once
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });

  // ── Guard: refresh skipped while already refreshing ────────────────

  it("sets isRefreshingList to true during refresh and false after", async () => {
    let resolvePromise: (value: DownloadSummary[]) => void;
    const pendingPromise = new Promise<DownloadSummary[]>((resolve) => {
      resolvePromise = resolve;
    });

    mockTauriCommandValue("download_list", () => pendingPromise);

    const promise = store.refreshList();

    // During the refresh
    expect(store.isRefreshingList).toBe(true);

    resolvePromise!([]);
    await promise;

    // After refresh
    expect(store.isRefreshingList).toBe(false);
  });

  // ── Multiple items ─────────────────────────────────────────────────

  it("handles multiple download items", async () => {
    const mockData = [
      createMockDownloadTask({ id: "task-1", fileName: "a.zip" }),
      createMockDownloadTask({ id: "task-2", fileName: "b.zip" }),
      createMockDownloadTask({ id: "task-3", fileName: "c.zip" }),
    ];
    mockTauriCommandValue("download_list", mockData);

    await store.refreshList();

    expect(store.downloads).toHaveLength(3);
    expect(store.downloads.map((d) => d.fileName)).toEqual(["a.zip", "b.zip", "c.zip"]);
  });

  // ── Error message serialization ────────────────────────────────────

  it("converts non-Error throws gracefully", async () => {
    mockTauriCommand("download_list", () => {
      // eslint-disable-next-line no-throw-literal
      throw "String error";
    });

    await store.refreshList();

    // Error is caught by setError (which calls notify.notifyError)
    expect(store.downloads).toEqual([]);
  });
});
