export type ChecksumMode = "none" | "blake3" | "sha256" | "xxh3_128";
export type ThreadMode = "fixed" | "adaptive";
export type AdaptiveProfile = "conservative" | "balanced" | "aggressive";
export type TaskKind = "http" | "bt" | "metalink" | "sftp";
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
  peerCount?: number;
  uploadStatus?: BtUploadStatus;
  error?: string;
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
  peerCount?: number;
  uploadStatus?: BtUploadStatus;
  createdAtMs: number;
  updatedAtMs: number;
  chunks?: ChunkInfo[];
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
}
