export type ChecksumMode = "none" | "blake3";

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
  maxConnections?: number;
  maxRetries?: number;
  checksum?: ChecksumMode;
}

export interface DownloadFormState {
  url: string;
  destinationDir: string;
  fileName: string;
  maxConnections: number | null;
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
