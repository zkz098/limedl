export type ChecksumMode = "none" | "blake3" | "sha256" | "xxh3_128";
export type ThreadMode = "fixed" | "adaptive";
export type AdaptiveProfile = "conservative" | "balanced" | "aggressive";

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
  url: string;
  destinationDir: string;
  fileName?: string;
  threadMode?: ThreadMode;
  threadCount?: number;
  maxRetries?: number;
  checksum?: ChecksumMode;
}

export interface DownloadFormState {
  url: string;
  destinationDir: string;
  fileName: string;
  threadMode: ThreadMode;
  threadCount: number | null;
  maxRetries: number | null;
  checksum: ChecksumMode;
}

export interface DownloadSummary {
  id: string;
  state: DownloadState;
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
  error?: string;
}

export interface DownloadSnapshot {
  id: string;
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
  createdAtMs: number;
  updatedAtMs: number;
}
