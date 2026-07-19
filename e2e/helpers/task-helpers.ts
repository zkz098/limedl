/**
 * Shared helper functions for creating mock download tasks in E2E tests.
 *
 * Provides a `seedDownloadTask` function that handles the full RPC lifecycle
 * when starting a download through the composer dialog, plus mock factory
 * functions for DownloadSummary and DownloadProgress payloads.
 */

import type { Page } from "@playwright/test";
import type { WsMocker } from "./ws-mocker";

/**
 * Create a download task through the UI composer dialog and mock the full RPC lifecycle.
 *
 * The frontend calls these RPC methods in sequence after clicking "Start download":
 *   1. download.start → respond with taskId
 *   2. download.list  → respond with empty array (or seeded data)
 *   3. download.status → respond with full DownloadSummary
 */
export async function seedDownloadTask(
  page: Page,
  wsMocker: WsMocker,
  taskId: string,
  options: { url?: string; fileName?: string; totalBytes?: number } = {},
): Promise<void> {
  const url = options.url ?? "http://127.0.0.1:9876/10mb.bin";
  const fileName = options.fileName ?? "10mb.bin";
  const totalBytes = options.totalBytes ?? 10_000_000;

  // Open composer dialog
  await page.getByRole("button", { name: "Add Task" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByPlaceholder("Paste a link or choose a torrent file").fill(url);

  // Wait for download.start
  const startPromise = wsMocker.waitForMethod("download.start");
  await dialog.getByRole("button", { name: "Start download" }).click();
  await startPromise;

  // Respond to download.start
  wsMocker.respondToMethod("download.start", { taskId });

  // Respond to download.list
  await wsMocker.waitForMethod("download.list");
  wsMocker.respondToMethod("download.list", []);

  // Respond to download.status
  await wsMocker.waitForMethod("download.status");
  wsMocker.respondToMethod("download.status", makeMockSummary(taskId, { url, fileName, totalBytes }));
}

/** Create a realistic mock DownloadSummary for test responses */
export function makeMockSummary(taskId: string, overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id: taskId,
    kind: "http",
    state: "downloading",
    url: "http://127.0.0.1:9876/10mb.bin",
    finalUrl: null,
    fileName: "10mb.bin",
    destinationPath: "/tmp/test/10mb.bin",
    tempPath: "/tmp/test/.10mb.bin.part",
    totalBytes: 10_000_000,
    downloadedBytes: 0,
    supportsRanges: true,
    connectionCount: 4,
    threadMode: "adaptive",
    requestedThreadCount: 4,
    desiredThreadCount: 4,
    allocatedThreadCount: 4,
    adaptiveProfile: null,
    threadNote: null,
    checksum: null,
    checksumMode: "none",
    etag: null,
    lastModified: null,
    error: null,
    speedBytesPerSecond: 0,
    etaSeconds: null,
    uploadedBytes: 0,
    uploadSpeedBytesPerSecond: 0,
    peerCount: 0,
    uploadStatus: null,
    infoHash: null,
    cdnAccelerated: false,
    seedCount: null,
    leechCount: null,
    downloadLimitBps: null,
    uploadLimitBps: null,
    mirrorUrl: null,
    chunks: [],
    createdAtMs: Date.now(),
    updatedAtMs: Date.now(),
    ...overrides,
  };
}

/** Create a mock DownloadProgress payload for progress events */
export function makeMockProgress(taskId: string, overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id: taskId,
    state: "downloading",
    downloadedBytes: 0,
    totalBytes: 10_000_000,
    speedBytesPerSecond: 0,
    etaSeconds: null,
    connectionCount: 4,
    allocatedThreadCount: 4,
    error: null,
    uploadedBytes: 0,
    uploadSpeedBytesPerSecond: 0,
    peerCount: 0,
    degraded: false,
    diskType: "ssd" as const,
    flushing: false,
    ...overrides,
  };
}
