import type { SortDirection, SortKey } from "./settings";

// ── Re-exported generated types (single source of truth from Rust) ──
export type {
  AdaptiveProfile,
  BtFileStatus,
  BtPeerInfo,
  BtPieceInfo,
  BtRuntimeStatus,
  BtTrackerInfo,
  BtUploadStatus,
  ChecksumMode,
  DownloadState,
  Priority,
  TaskKind,
  ThreadMode,
  TorrentFileEntry,
} from "./generated/types";

// Import Rust types with aliases for local extension
import type {
  ChunkInfo as RustChunkInfo,
  ChecksumMode as _ChecksumMode,
  DownloadProgress as RustDownloadProgress,
  DownloadSnapshot as RustDownloadSnapshot,
  DownloadSummary as RustDownloadSummary,
  StartDownloadRequest as RustStartDownloadRequest,
  TaskKind as _TaskKind,
  ThreadMode as _ThreadMode,
} from "./generated/types";

// ── Frontend-extended types (compatible with existing code) ──

/** DownloadSummary with additional runtime-only fields used by frontend. */
export interface DownloadSummary extends RustDownloadSummary {
  degraded?: boolean;
  diskType?: "ssd" | "hdd";
  flushing?: boolean;
}
/** DownloadSnapshot with same variant. */
export interface DownloadSnapshot extends RustDownloadSnapshot {}

/** DownloadProgress with same variant. */
export type DownloadProgress = RustDownloadProgress;

/** StartDownloadRequest with same variant. */
export interface StartDownloadRequest extends RustStartDownloadRequest {}

/** ChunkInfo with same variant. */
export type ChunkInfo = RustChunkInfo;

// ── Pure frontend types (no Rust counterpart) ──

export interface DownloadFormState {
  kind: _TaskKind;
  url: string;
  destinationDir: string;
  fileName: string;
  userAgent: string;
  threadMode: _ThreadMode;
  threadCount: number | null;
  maxRetries: number | null;
  checksum: _ChecksumMode;
  downloadLimitBps: number | null;
  uploadLimitBps: number | null;
  selectedFileIndices?: number[];
  startPaused?: boolean;
}

export interface ViewOptions {
  sortKey: SortKey;
  sortDirection: SortDirection;
  compactView: boolean;
  visibleColumns: string[];
}

export interface MultiSelectState {
  multiSelectMode: boolean;
  selectedIds: Set<string>;
  removedDownloadIds: string[];
}

export interface BatchUrlEntry {
  id: string;
  url: string;
  kind: _TaskKind;
  fileName: string;
  status: "ready" | "queued" | "success" | "error";
  error?: string;
}

export interface BatchSubmitProgress {
  done: number;
  total: number;
}
