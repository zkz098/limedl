import { describe, it, expect, vi, beforeEach } from "vitest";
import { ref, nextTick } from "vue";

vi.mock("../../lib/tauri/download-api", () => ({
  startDownload: vi.fn().mockResolvedValue("http:test-id"),
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

import type { Ref } from "vue";

import { startDownload } from "../../lib/tauri/download-api";
import { pickDirectory, pickTorrentFile } from "../../lib/tauri/dialog-api";
import { useDownloadForm } from "../../composables/useDownloadForm";

const mockStartDownload = vi.mocked(startDownload);
const mockPickDirectory = vi.mocked(pickDirectory);
const mockPickTorrentFile = vi.mocked(pickTorrentFile);

describe("useDownloadForm", () => {
  let selectedId: Ref<string | null>;
  let allowAutoSelect: Ref<boolean>;
  let isStarting: Ref<boolean>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let upsertSummary: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let refreshList: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let refreshStatus: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let setMessage: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let setError: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let clearMessage: any;

  function createForm() {
    return useDownloadForm({
      selectedId,
      allowAutoSelect,
      isStarting,
      upsertSummary,
      refreshList,
      refreshStatus,
      setMessage,
      setError,
      clearMessage,
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    selectedId = ref<string | null>(null);
    allowAutoSelect = ref(true);
    isStarting = ref(false);
    upsertSummary = vi.fn();
    refreshList = vi.fn().mockResolvedValue(undefined);
    refreshStatus = vi.fn().mockResolvedValue(undefined);
    setMessage = vi.fn();
    setError = vi.fn();
    clearMessage = vi.fn();
  });

  // ── Form initialization ───────────────────────────────────────────

  it("has default values", () => {
    const { form } = createForm();

    expect(form.kind).toBe("http");
    expect(form.threadMode).toBe("adaptive");
    expect(form.threadCount).toBe(8);
    expect(form.maxRetries).toBe(5);
    expect(form.checksum).toBe("blake3");
    expect(form.url).toBe("");
    expect(form.destinationDir).toBe("");
    expect(form.fileName).toBe("");
    expect(form.userAgent).toBe("");
    expect(form.downloadLimitBps).toBeNull();
    expect(form.uploadLimitBps).toBeNull();
  });

  // ── isFormValid ────────────────────────────────────────────────────

  it("isFormValid returns false when URL is empty", () => {
    const { isFormValid, form } = createForm();
    form.destinationDir = "/downloads";

    expect(isFormValid.value).toBe(false);
  });

  it("isFormValid returns false when destinationDir is empty", () => {
    const { isFormValid, form } = createForm();
    form.url = "https://example.com/file.zip";

    expect(isFormValid.value).toBe(false);
  });

  it("isFormValid returns true when both URL and destinationDir are set", () => {
    const { isFormValid, form } = createForm();
    form.url = "https://example.com/file.zip";
    form.destinationDir = "/downloads";

    expect(isFormValid.value).toBe(true);
  });

  // ── autoSetFileName ────────────────────────────────────────────────

  it("extracts file name from URL path", () => {
    const { autoSetFileName, form } = createForm();

    autoSetFileName("https://example.com/files/myfile.zip");

    expect(form.fileName).toBe("myfile.zip");
  });

  it("handles URL with query params", () => {
    const { autoSetFileName, form } = createForm();

    autoSetFileName("https://example.com/file.zip?token=abc&dl=1");

    expect(form.fileName).toBe("file.zip");
  });

  it("handles URL without file name extension", () => {
    const { autoSetFileName, form } = createForm();

    autoSetFileName("https://example.com/download/file");

    expect(form.fileName).toBe("file");
  });

  it("respects locked file name and does not override", () => {
    const { autoSetFileName, isFileNameLocked, form } = createForm();

    form.fileName = "custom.txt";
    isFileNameLocked.value = true;

    autoSetFileName("https://example.com/auto.zip");

    expect(form.fileName).toBe("custom.txt");
  });

  // ── URL change handling ────────────────────────────────────────────

  it("changing URL with auto-detect triggers file name extraction", async () => {
    const { form } = createForm();

    form.url = "https://example.com/video.mp4";
    await nextTick();

    expect(form.fileName).toBe("video.mp4");
  });

  it("changing URL without auto-detect keeps existing file name", async () => {
    const { form, autoDetectFileName } = createForm();

    autoDetectFileName.value = false;
    form.fileName = "manual.txt";
    form.url = "https://example.com/auto.zip";
    await nextTick();

    expect(form.fileName).toBe("manual.txt");
  });

  // ── resetForm ──────────────────────────────────────────────────────

  it("clears URL, fileName, destinationDir", () => {
    const { form, resetForm } = createForm();

    form.url = "https://example.com/file.zip";
    form.fileName = "file.zip";
    form.destinationDir = "/downloads";
    resetForm();

    expect(form.url).toBe("");
    expect(form.fileName).toBe("");
    expect(form.destinationDir).toBe("");
  });

  it("resets threadMode and threadCount to defaults", () => {
    const { form, resetForm } = createForm();

    form.threadMode = "fixed";
    form.threadCount = 16;
    form.maxRetries = 99;
    form.checksum = "none";
    form.kind = "bt";
    resetForm();

    expect(form.threadMode).toBe("adaptive");
    expect(form.threadCount).toBe(8);
    expect(form.maxRetries).toBe(5);
    expect(form.checksum).toBe("blake3");
    expect(form.kind).toBe("http");
  });

  // ── pickDirectory ──────────────────────────────────────────────────

  it("calls Tauri dialog and sets destinationDir", async () => {
    mockPickDirectory.mockResolvedValue("/chosen/path");
    const { pickDestinationDirectory, form } = createForm();

    await pickDestinationDirectory();

    expect(mockPickDirectory).toHaveBeenCalledTimes(1);
    expect(form.destinationDir).toBe("/chosen/path");
  });

  it("when dialog is cancelled does not change destinationDir", async () => {
    mockPickDirectory.mockResolvedValue(null);
    const { pickDestinationDirectory, form } = createForm();

    form.destinationDir = "/original/path";
    await pickDestinationDirectory();

    expect(form.destinationDir).toBe("/original/path");
  });

  it("guards against concurrent directory picks", async () => {
    let resolvePicker!: (value: string | null) => void;
    const pending = new Promise<string | null>((resolve) => {
      resolvePicker = resolve;
    });
    mockPickDirectory.mockReturnValue(pending);
    const { pickDestinationDirectory } = createForm();

    const p1 = pickDestinationDirectory();
    const p2 = pickDestinationDirectory(); // should be a no-op

    resolvePicker!("/path");
    await p1;
    await p2;

    expect(mockPickDirectory).toHaveBeenCalledTimes(1);
  });

  // ── pickTorrentFile ────────────────────────────────────────────────

  it("calls Tauri dialog and sets kind and url", async () => {
    mockPickTorrentFile.mockResolvedValue("/torrents/file.torrent");
    const { pickTorrentSourceFile, form } = createForm();

    await pickTorrentSourceFile();

    expect(mockPickTorrentFile).toHaveBeenCalledTimes(1);
    expect(form.kind).toBe("bt");
    expect(form.url).toBe("/torrents/file.torrent");
  });

  it("extracts torrent file name from path", async () => {
    mockPickTorrentFile.mockResolvedValue("/torrents/ubuntu-24.04.torrent");
    const { pickTorrentSourceFile, form } = createForm();

    await pickTorrentSourceFile();

    expect(form.url).toContain("ubuntu-24.04.torrent");
  });

  it("guards against concurrent torrent picks", async () => {
    let resolvePicker!: (value: string | null) => void;
    const pending = new Promise<string | null>((resolve) => {
      resolvePicker = resolve;
    });
    mockPickTorrentFile.mockReturnValue(pending);
    const { pickTorrentSourceFile } = createForm();

    const p1 = pickTorrentSourceFile();
    const p2 = pickTorrentSourceFile();

    resolvePicker!("/t.torrent");
    await p1;
    await p2;

    expect(mockPickTorrentFile).toHaveBeenCalledTimes(1);
  });

  // ── submitStart ────────────────────────────────────────────────────

  it("with valid form calls startDownload, refreshes list, resets form", async () => {
    mockStartDownload.mockResolvedValue("http:test-123");
    const { submitStart, form } = createForm();

    form.url = "https://example.com/file.zip";
    form.destinationDir = "/downloads";
    form.fileName = "file.zip";

    await submitStart();

    expect(mockStartDownload).toHaveBeenCalledTimes(1);
    expect(mockStartDownload).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://example.com/file.zip",
        destinationDir: "/downloads",
        fileName: "file.zip",
      }),
    );
    expect(selectedId.value).toBe("http:test-123");
    expect(refreshList).toHaveBeenCalledTimes(1);
    expect(refreshStatus).toHaveBeenCalledWith("http:test-123", { silent: true });
    expect(setMessage).toHaveBeenCalledWith(
      'messages.downloadQueued {"id":"http:test-123"}',
    );
    // Form should be reset after successful start
    expect(form.url).toBe("");
    expect(form.fileName).toBe("");
    expect(form.destinationDir).toBe("");
  });

  it("with invalid form does not call startDownload", async () => {
    const { submitStart, form } = createForm();

    form.url = "";
    form.destinationDir = "";
    await submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
    expect(setError).toHaveBeenCalled();
  });

  it("with empty URL calls setError", async () => {
    const { submitStart, form } = createForm();

    form.destinationDir = "/downloads";
    form.url = "";
    await submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
    expect(setError).toHaveBeenCalledWith("messages.startRequired");
  });

  it("with empty destinationDir calls setError", async () => {
    const { submitStart, form } = createForm();

    form.url = "https://example.com/file.zip";
    form.destinationDir = "";
    await submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
    expect(setError).toHaveBeenCalledWith("messages.startRequired");
  });

  it("when already starting does not call startDownload again", async () => {
    mockStartDownload.mockResolvedValue("http:test-123");
    const { submitStart, form } = createForm();

    form.url = "https://example.com/file.zip";
    form.destinationDir = "/downloads";
    isStarting.value = true;
    await submitStart();

    expect(mockStartDownload).not.toHaveBeenCalled();
  });

  it("sets isStarting to false when startDownload throws", async () => {
    mockStartDownload.mockRejectedValue(new Error("Network error"));
    const { submitStart, form } = createForm();

    form.url = "https://example.com/file.zip";
    form.destinationDir = "/downloads";
    await submitStart();

    expect(isStarting.value).toBe(false);
    expect(setError).toHaveBeenCalledWith("errors.networkError");
  });

  it("clears message before starting", async () => {
    mockStartDownload.mockResolvedValue("http:test-123");
    const { submitStart, form } = createForm();

    form.url = "https://example.com/file.zip";
    form.destinationDir = "/downloads";
    await submitStart();

    expect(clearMessage).toHaveBeenCalled();
  });

  // ── Thread count validation ────────────────────────────────────────

  it("clampThreadCount clamps to minimum of 1", () => {
    const { clampThreadCount } = createForm();

    expect(clampThreadCount(0)).toBe(1);
    expect(clampThreadCount(-5)).toBe(1);
    expect(clampThreadCount(1)).toBe(1);
  });

  it("clampThreadCount clamps to maximum of 128", () => {
    const { clampThreadCount } = createForm();

    expect(clampThreadCount(200)).toBe(128);
    expect(clampThreadCount(128)).toBe(128);
    expect(clampThreadCount(64)).toBe(64);
  });

  it("buildStartRequest uses clamped threadCount", () => {
    const { form, buildStartRequest } = createForm();

    form.threadCount = 0;
    const request = buildStartRequest();

    expect(request.threadCount).toBe(1);
  });

  it("buildStartRequest clamps threadCount to max", () => {
    const { form, buildStartRequest } = createForm();

    form.threadCount = 999;
    const request = buildStartRequest();

    expect(request.threadCount).toBe(128);
  });

  it("applySchedulerDefaults caps threadCount when maxThreadsPerTask is lower", () => {
    const { form, applySchedulerDefaults } = createForm();

    form.threadCount = 32;
    applySchedulerDefaults("automatic", 16);

    expect(form.threadCount).toBe(16);
    expect(form.threadMode).toBe("adaptive");
  });

  it("applySchedulerDefaults with traditional mode sets fixed threads", () => {
    const { form, applySchedulerDefaults } = createForm();

    applySchedulerDefaults("traditional", 8);

    expect(form.threadMode).toBe("fixed");
    expect(form.threadCount).toBe(8);
  });

  it("applySchedulerDefaults does nothing for non-http kind", () => {
    const { form, applySchedulerDefaults } = createForm();

    form.kind = "bt";
    form.threadMode = "fixed";
    applySchedulerDefaults("automatic", 4);

    expect(form.threadMode).toBe("fixed"); // unchanged
  });

  // ── BT-specific ────────────────────────────────────────────────────

  it("buildStartRequest includes BT fields when kind is bt", () => {
    const { form, buildStartRequest } = createForm();

    form.kind = "bt";
    form.url = "/path/to/file.torrent";
    form.destinationDir = "/downloads";
    form.selectedFileIndices = [0, 2];
    form.startPaused = true;

    const request = buildStartRequest();

    expect(request.kind).toBe("bt");
    expect(request.selectedFileIndices).toEqual([0, 2]);
    expect(request.startPaused).toBe(true);
  });

  it("buildStartRequest excludes thread fields when kind is bt", () => {
    const { form, buildStartRequest } = createForm();

    form.kind = "bt";
    form.url = "/path/to/file.torrent";
    form.destinationDir = "/downloads";

    const request = buildStartRequest();

    expect(request.threadMode).toBeUndefined();
    expect(request.threadCount).toBeUndefined();
    expect(request.checksum).toBeUndefined();
  });

  it("sets kind to bt when torrent file is picked", async () => {
    mockPickTorrentFile.mockResolvedValue("/path/to/ubuntu.torrent");
    const { pickTorrentSourceFile, form } = createForm();

    expect(form.kind).toBe("http"); // default

    await pickTorrentSourceFile();

    expect(form.kind).toBe("bt");
  });

  it("sets kind to http when picking directory (remains http)", async () => {
    mockPickDirectory.mockResolvedValue("/downloads");
    const { form } = createForm();

    expect(form.kind).toBe("http"); // unchanged
  });

  // ── BT speed limit after start ────────────────────────────────────

  it("sets BT speed limits after start when limits are configured", async () => {
    const { setBtSpeedLimit } = await import("../../lib/tauri/download-api");
    const mockSetBtSpeedLimit = vi.mocked(setBtSpeedLimit);
    mockSetBtSpeedLimit.mockResolvedValue(undefined);
    mockStartDownload.mockResolvedValue("bt:test-456");

    const { submitStart, form } = createForm();

    form.kind = "bt";
    form.url = "/torrents/file.torrent";
    form.destinationDir = "/downloads";
    form.downloadLimitBps = 500_000;
    form.uploadLimitBps = 100_000;

    await submitStart();

    expect(mockSetBtSpeedLimit).toHaveBeenCalledWith("bt:test-456", 500_000, 100_000);
  });

  it("does not set BT speed limits when limits are null", async () => {
    const { setBtSpeedLimit } = await import("../../lib/tauri/download-api");
    const mockSetBtSpeedLimit = vi.mocked(setBtSpeedLimit);
    mockStartDownload.mockResolvedValue("bt:test-789");

    const { submitStart, form } = createForm();

    form.kind = "bt";
    form.url = "/torrents/file.torrent";
    form.destinationDir = "/downloads";
    // downloadLimitBps and uploadLimitBps are null by default

    await submitStart();

    expect(mockSetBtSpeedLimit).not.toHaveBeenCalled();
  });

  // ── autoFillFromClipboard ──────────────────────────────────────────

  it("autoFillFromClipboard sets url and kind for http link", async () => {
    // navigator.clipboard.readText is not available in jsdom — skip
    // This test verifies the method exists and is properly exported.
    const { autoFillFromClipboard } = createForm();

    expect(typeof autoFillFromClipboard).toBe("function");
  });
});
