export type ChecksumMode = "none" | "blake3" | "sha256" | "xxh3_128";
export type ThreadMode = "fixed" | "adaptive";
export type AdaptiveProfile = "conservative" | "balanced" | "aggressive";
export type TaskKind = "http" | "bt";
export type BtUploadStatus = "idle" | "uploading" | "paused" | "paused_by_limit";

export type DownloadState =
  | "queued"
  | "downloading"
  | "paused"
  | "retrying"
  | "verifying"
  | "completed"
  | "failed"
  | "canceled";

export interface StartDownloadRequest {
  kind?: TaskKind;
  url: string;
  destinationDir: string;
  fileName?: string;
  userAgent?: string;
  threadMode?: ThreadMode;
  threadCount?: number;
  maxRetries?: number;
  checksum?: ChecksumMode;
  selectedFileIndices?: number[];
  startPaused?: boolean;
}

export interface DownloadFormState {
  kind: TaskKind;
  url: string;
  destinationDir: string;
  fileName: string;
  userAgent: string;
  threadMode: ThreadMode;
  threadCount: number | null;
  maxRetries: number | null;
  checksum: ChecksumMode;
  downloadLimitBps: number | null;
  uploadLimitBps: number | null;
  selectedFileIndices?: number[];
  startPaused?: boolean;
}

export interface DownloadSummary {
  id: string;
  kind: TaskKind;
  state: DownloadState;
  url: string;
  fileName: string;
  destinationPath: string;
  totalBytes?: number;
  downloadedBytes: number;
  connectionCount: number;
  threadMode: ThreadMode;
  requestedThreadCount?: number;
  desiredThreadCount?: number;
  allocatedThreadCount?: number;
  adaptiveProfile?: AdaptiveProfile;
  threadNote?: string;
  speedBytesPerSecond?: number;
  etaSeconds?: number;
  uploadedBytes?: number;
  uploadSpeedBytesPerSecond?: number;
  peerCount?: number;
  uploadStatus?: BtUploadStatus;
  infoHash?: string;
  error?: string;
  cdnAccelerated?: boolean;
  createdAtMs: number;
  seedCount?: number;
  leechCount?: number;
  downloadLimitBps?: number;
  uploadLimitBps?: number;
  chunks?: ChunkInfo[];
}

export interface ChunkInfo {
  index: number;
  start: number;
  end: number;
  downloaded: number;
  completed: boolean;
  claimedBy: number | null;
}

export interface DownloadSnapshot {
  id: string;
  kind: TaskKind;
  state: DownloadState;
  url: string;
  finalUrl: string;
  fileName: string;
  destinationPath: string;
  tempPath: string;
  totalBytes?: number;
  downloadedBytes: number;
  supportsRanges: boolean;
  connectionCount: number;
  threadMode: ThreadMode;
  requestedThreadCount?: number;
  desiredThreadCount?: number;
  allocatedThreadCount?: number;
  adaptiveProfile?: AdaptiveProfile;
  threadNote?: string;
  checksum?: string;
  checksumMode: ChecksumMode;
  etag?: string;
  lastModified?: string;
  error?: string;
  speedBytesPerSecond?: number;
  etaSeconds?: number;
  uploadedBytes?: number;
  uploadSpeedBytesPerSecond?: number;
  peerCount?: number;
  uploadStatus?: BtUploadStatus;
  infoHash?: string;
  createdAtMs: number;
  updatedAtMs: number;
  cdnAccelerated?: boolean;
  chunks?: ChunkInfo[];
  seedCount?: number;
  leechCount?: number;
  downloadLimitBps?: number;
  uploadLimitBps?: number;
}

export interface BtRuntimeStatus {
  connected: boolean;
  dhtEnabled: boolean;
  dhtNodes?: number;
  torrentCount: number;
  peerCount: number;
  uploadSpeedBytesPerSecond?: number;
  uploadedBytes: number;
  updatedAtMs: number;
  seedCount?: number;
  leechCount?: number;
}

export interface TorrentFileEntry {
  index: number;
  path: string;
  size: number;
}

export interface BtPeerInfo {
  address: string;
  client: string;
  flags: string;
  downloadSpeed: number;
  uploadSpeed: number;
  progress: number;
}

export interface BtTrackerInfo {
  url: string;
}

export interface BtPieceInfo {
  index: number;
  completed: boolean;
}

export interface BtFileStatus {
  index: number;
  path: string;
  size: number;
  downloadedBytes: number;
  included: boolean;
}
