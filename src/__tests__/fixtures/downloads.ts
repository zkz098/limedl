/**
 * Factory functions for producing mock download task objects matching
 * the DownloadSummary / DownloadSnapshot types from src/types/download.ts.
 */

import type {
  DownloadSnapshot,
  DownloadState,
  DownloadSummary,
  TaskKind,
  ThreadMode,
} from "../../types/download";

let nextIdCounter = 1;

function nextId(): string {
  return `mock-${nextIdCounter++}-${Date.now()}`;
}

/** Default values for a minimal DownloadSummary. */
const defaultSummary: DownloadSummary = {
  id: "",
  kind: "http",
  state: "queued",
  url: "https://example.com/file.zip",
  fileName: "file.zip",
  destinationPath: "C:\\Downloads",
  downloadedBytes: 0,
  connectionCount: 0,
  threadMode: "fixed",
  totalBytes: null,
  requestedThreadCount: null,
  desiredThreadCount: null,
  allocatedThreadCount: null,
  adaptiveProfile: null,
  threadNote: null,
  speedBytesPerSecond: 0,
  etaSeconds: null,
  uploadedBytes: null,
  uploadSpeedBytesPerSecond: null,
  peerCount: null,
  uploadStatus: null,
  infoHash: null,
  error: null,
  cdnAccelerated: false,
  createdAtMs: Date.now(),
  priority: "normal",
};

/**
 * Create a mock DownloadSummary with sensible defaults.
 * Pass `overrides` to customize specific fields.
 *
 * @example
 * ```ts
 * const task = createMockDownloadTask({ state: "downloading", downloadedBytes: 500 });
 * ```
 */
export function createMockDownloadTask(overrides?: Partial<DownloadSummary>): DownloadSummary {
  const id = overrides?.id ?? nextId();

  return {
    ...defaultSummary,
    id,
    ...overrides,
  };
}

/**
 * Create an array of mock DownloadSummary objects with sequential IDs.
 *
 * @param count - Number of mock tasks to generate.
 * @param overrides - Optional overrides applied to ALL tasks in the list.
 */
export function createMockDownloadList(
  count: number,
  overrides?: Partial<DownloadSummary>,
): DownloadSummary[] {
  return Array.from({ length: count }, (_, i) =>
    createMockDownloadTask({
      id: `mock-${i + 1}`,
      ...overrides,
    }),
  );
}

/** Convenience presets for common download states. */
export const DownloadPresets = {
  queued(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({ state: "queued", ...overrides });
  },

  downloading(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      state: "downloading",
      downloadedBytes: 1024 * 1024,
      totalBytes: 10 * 1024 * 1024,
      speedBytesPerSecond: 500 * 1024,
      etaSeconds: 18,
      connectionCount: 4,
      ...overrides,
    });
  },

  paused(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      state: "paused",
      downloadedBytes: 5 * 1024 * 1024,
      totalBytes: 10 * 1024 * 1024,
      ...overrides,
    });
  },

  completed(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      state: "completed",
      downloadedBytes: 10 * 1024 * 1024,
      totalBytes: 10 * 1024 * 1024,
      ...overrides,
    });
  },

  failed(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      state: "failed",
      error: "Connection reset by peer",
      downloadedBytes: 3 * 1024 * 1024,
      totalBytes: 10 * 1024 * 1024,
      ...overrides,
    });
  },

  canceled(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      state: "canceled",
      downloadedBytes: 2 * 1024 * 1024,
      totalBytes: 10 * 1024 * 1024,
      ...overrides,
    });
  },

  torrent(overrides?: Partial<DownloadSummary>): DownloadSummary {
    return createMockDownloadTask({
      kind: "bt",
      state: "downloading",
      url: "magnet:?xt=urn:btih:deadbeef",
      fileName: "ubuntu-24.04-desktop.iso",
      downloadedBytes: 2 * 1024 * 1024 * 1024,
      totalBytes: 5 * 1024 * 1024 * 1024,
      speedBytesPerSecond: 10 * 1024 * 1024,
      etaSeconds: 300,
      peerCount: 42,
      uploadedBytes: 500 * 1024 * 1024,
      uploadSpeedBytesPerSecond: 2 * 1024 * 1024,
      infoHash: "deadbeefcafebabedeadbeefcafebabedeadbeef",
      seedCount: 15,
      leechCount: 27,
      ...overrides,
    });
  },
};

/**
 * Create a mock DownloadSnapshot (detailed task info).
 * Has more fields than DownloadSummary.
 */
export function createMockDownloadSnapshot(
  overrides?: Partial<DownloadSnapshot>,
): DownloadSnapshot {
  const id = overrides?.id ?? nextId();

  return {
    id,
    kind: "http" as TaskKind,
    state: "downloading" as DownloadState,
    url: "https://example.com/file.zip",
    finalUrl: "https://example.com/file.zip",
    fileName: "file.zip",
    destinationPath: "C:\\Downloads",
    tempPath: "C:\\Downloads\\.file.zip.part",
    downloadedBytes: 1024 * 1024,
    totalBytes: 10 * 1024 * 1024,
    supportsRanges: true,
    connectionCount: 4,
    threadMode: "fixed" as ThreadMode,
    checksumMode: "blake3",
    cdnAccelerated: false,
    degraded: false,
    flushing: false,
    speedBytesPerSecond: 500 * 1024,
    etaSeconds: 18,
    createdAtMs: Date.now() - 60_000,
    updatedAtMs: Date.now(),
    priority: "normal",
    ...overrides,
  } as DownloadSnapshot;
}

/**
 * Reset the internal ID counter (useful in beforeEach for deterministic IDs).
 */
export function resetMockIds() {
  nextIdCounter = 1;
}
