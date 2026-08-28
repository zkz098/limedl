import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("../../lib/tauri/download-api", () => ({
  startDownload: vi.fn().mockResolvedValue({ kind: "http", id: "test-id" }),
  setBtSpeedLimit: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../lib/tauri/dialog-api", () => ({
  pickDirectory: vi.fn(),
  pickTorrentFile: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) {
      return `${key} ${JSON.stringify(options)}`;
    }
    return key;
  }),
}));

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

import { startDownload } from "../../lib/tauri/download-api";
import { pickDirectory, pickTorrentFile } from "../../lib/tauri/dialog-api";
import { useDownloadStore } from "../../stores/download/index";

const mockStartDownload = vi.mocked(startDownload);
const mockPickDirectory = vi.mocked(pickDirectory);
const mockPickTorrentFile = vi.mocked(pickTorrentFile);

describe("useDownloadStore (form)", () => {
  let store: ReturnType<typeof useDownloadStore>;

  beforeEach(() => {
    vi.clearAllMocks();
    setActivePinia(createPinia());
    store = useDownloadStore();
  });

  afterEach(() => {
    store.destroyStore();
  });

  // ── Form initialization ───────────────────────────────────────────

  it("has default values", () => {
    expect(store.form.kind).toBe("http");
    expect(store.form.threadMode).toBe("adaptive");
    expect(store.form.threadCount).toBe(8);
    expect(store.form.maxRetries).toBe(5);
    expect(store.form.checksum).toBe("blake3");
    expect(store.form.url).toBe("");
    expect(store.form.destinationDir).toBe("");
    expect(store.form.fileName).toBe("");
    expect(store.form.userAgent).toBe("");
    expect(store.form.downloadLimitBps).toBeNull();
    expect(store.form.uploadLimitBps).toBeNull();
  });

  // ── resetForm (exercised via submitStart) ─────────────────────────

  it("clears form fields after submit", async () => {
    mockStartDownload.mockResolvedValue({ kind: "http", id: "test-123" });

    store.form.url = "https://example.com/file.zip";
    store.form.destinationDir = "/downloads";
    store.form.fileName = "file.zip";

    await store.submitStart();

    expect(store.form.url).toBe("");
    expect(store.form.fileName).toBe("");
    expect(store.form.destinationDir).toBe("");
  });

  // ── pickDirectory ──────────────────────────────────────────────────

  it("calls Tauri dialog and sets destinationDir", async () => {
    mockPickDirectory.mockResolvedValue("/chosen/path");

    await store.pickDestinationDirectory();

    expect(mockPickDirectory).toHaveBeenCalledTimes(1);
    expect(store.form.destinationDir).toBe("/chosen/path");
  });

  it("when dialog is cancelled does not change destinationDir", async () => {
    mockPickDirectory.mockResolvedValue(null);

    store.form.destinationDir = "/original/path";
    await store.pickDestinationDirectory();

    expect(store.form.destinationDir).toBe("/original/path");
  });

  it("guards against concurrent directory picks", async () => {
    let resolvePicker!: (value: string | null) => void;
    const pending = new Promise<string | null>((resolve) => {
      resolvePicker = resolve;
    });
    mockPickDirectory.mockReturnValue(pending);

    const p1 = store.pickDestinationDirectory();
    const p2 = store.pickDestinationDirectory(); // should be a no-op

    resolvePicker("/path");
    await p1;
    await p2;

    expect(mockPickDirectory).toHaveBeenCalledTimes(1);
  });

  // ── pickTorrentFile ────────────────────────────────────────────────

  it("calls Tauri dialog and sets kind and url", async () => {
    mockPickTorrentFile.mockResolvedValue("/torrents/file.torrent");

    await store.pickTorrentSourceFile();

    expect(mockPickTorrentFile).toHaveBeenCalledTimes(1);
    expect(store.form.kind).toBe("bt");
    expect(store.form.url).toBe("/torrents/file.torrent");
  });

  it("guards against concurrent torrent picks", async () => {
    let resolvePicker!: (value: string | null) => void;
    const pending = new Promise<string | null>((resolve) => {
      resolvePicker = resolve;
    });
    mockPickTorrentFile.mockReturnValue(pending);

    const p1 = store.pickTorrentSourceFile();
    const p2 = store.pickTorrentSourceFile();

    resolvePicker("/t.torrent");
    await p1;
    await p2;

    expect(mockPickTorrentFile).toHaveBeenCalledTimes(1);
  });

  // ── submitStart ────────────────────────────────────────────────────

  it("with valid form calls startDownload and refreshes list", async () => {
    mockStartDownload.mockResolvedValue({ kind: "http", id: "test-123" });

    store.form.url = "https://example.com/file.zip";
    store.form.destinationDir = "/downloads";
    store.form.fileName = "file.zip";

    await store.submitStart();

    expect(mockStartDownload).toHaveBeenCalledTimes(1);
    expect(mockStartDownload).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://example.com/file.zip",
        destinationDir: "/downloads",
        fileName: "file.zip",
      }),
    );
  });

  it("with invalid form does not call startDownload", async () => {
    store.form.url = "";
    store.form.destinationDir = "";
    await store.submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
  });

  it("with empty URL does not call startDownload", async () => {
    store.form.destinationDir = "/downloads";
    store.form.url = "";
    await store.submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
  });

  it("with empty destinationDir does not call startDownload", async () => {
    store.form.url = "https://example.com/file.zip";
    store.form.destinationDir = "";
    await store.submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
  });

  it("when already starting does not call startDownload again", async () => {
    mockStartDownload.mockResolvedValue({ kind: "http", id: "test-123" });
    store.form.url = "https://example.com/file.zip";
    store.form.destinationDir = "/downloads";
    store.isStarting = true;
    await store.submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
  });

  it("sets isStarting to false when startDownload throws", async () => {
    mockStartDownload.mockRejectedValue(new Error("Network error"));
    store.form.url = "https://example.com/file.zip";
    store.form.destinationDir = "/downloads";
    await store.submitStart();

    expect(store.isStarting).toBe(false);
  });

  // ── applySchedulerDefaults ────────────────────────────────────────

  it("applySchedulerDefaults caps threadCount when maxThreadsPerTask is lower", () => {
    store.form.threadCount = 32;
    store.applySchedulerDefaults("automatic", 16);

    expect(store.form.threadCount).toBe(16);
    expect(store.form.threadMode).toBe("adaptive");
  });

  it("applySchedulerDefaults with traditional mode sets fixed threads", () => {
    store.applySchedulerDefaults("traditional", 8);

    expect(store.form.threadMode).toBe("fixed");
    expect(store.form.threadCount).toBe(8);
  });

  it("applySchedulerDefaults does nothing for non-http kind", () => {
    store.form.kind = "bt";
    store.form.threadMode = "fixed";
    store.applySchedulerDefaults("automatic", 4);

    expect(store.form.threadMode).toBe("fixed"); // unchanged
  });

  // ── BT speed limit after start ────────────────────────────────────

  it("sets BT speed limits after start when limits are configured", async () => {
    const { setBtSpeedLimit } = await import("../../lib/tauri/download-api");
    const mockSetBtSpeedLimit = vi.mocked(setBtSpeedLimit);
    mockSetBtSpeedLimit.mockResolvedValue(undefined);
    mockStartDownload.mockResolvedValue({ kind: "bt", id: "test-456" });

    store.form.kind = "bt";
    store.form.url = "/torrents/file.torrent";
    store.form.destinationDir = "/downloads";
    store.form.downloadLimitBps = 500_000;
    store.form.uploadLimitBps = 100_000;

    await store.submitStart();

    expect(mockSetBtSpeedLimit).toHaveBeenCalledWith("test-456", 500_000, 100_000);
  });

  it("does not set BT speed limits when limits are null", async () => {
    const { setBtSpeedLimit } = await import("../../lib/tauri/download-api");
    const mockSetBtSpeedLimit = vi.mocked(setBtSpeedLimit);
    mockStartDownload.mockResolvedValue({ kind: "bt", id: "test-789" });

    store.form.kind = "bt";
    store.form.url = "/torrents/file.torrent";
    store.form.destinationDir = "/downloads";

    await store.submitStart();

    expect(mockSetBtSpeedLimit).not.toHaveBeenCalled();
  });

  // ── autoFillFromClipboard ──────────────────────────────────────────

  it("autoFillFromClipboard exists as a method", () => {
    expect(typeof store.autoFillFromClipboard).toBe("function");
  });

  // ── Batch mode ────────────────────────────────────────────────────

  it("batch mode toggles correctly", () => {
    expect(store.batchMode).toBe(false);
    store.toggleBatchMode();
    expect(store.batchMode).toBe(true);
    store.toggleBatchMode();
    expect(store.batchMode).toBe(false);
  });
});
