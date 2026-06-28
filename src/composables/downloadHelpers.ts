import type { DownloadSnapshot, DownloadState, DownloadSummary } from "../types/download";

export const terminalStates: DownloadState[] = ["completed", "failed", "canceled"];
export const autoRefreshIntervalMs = 1500;

export function canPauseState(state?: DownloadState | null) {
  return Boolean(state && ["queued", "downloading", "retrying", "verifying"].includes(state));
}

export function canResumeState(state?: DownloadState | null) {
  return state === "paused" || state === "failed";
}

export function toMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

export function toSummary(snapshot: DownloadSnapshot): DownloadSummary {
  return {
    id: snapshot.id,
    kind: snapshot.kind,
    state: snapshot.state,
    url: snapshot.url,
    fileName: snapshot.fileName,
    destinationPath: snapshot.destinationPath,
    totalBytes: snapshot.totalBytes,
    downloadedBytes: snapshot.downloadedBytes,
    connectionCount: snapshot.connectionCount,
    threadMode: snapshot.threadMode,
    requestedThreadCount: snapshot.requestedThreadCount,
    desiredThreadCount: snapshot.desiredThreadCount,
    allocatedThreadCount: snapshot.allocatedThreadCount,
    adaptiveProfile: snapshot.adaptiveProfile,
    threadNote: snapshot.threadNote,
    speedBytesPerSecond: snapshot.speedBytesPerSecond,
    etaSeconds: snapshot.etaSeconds,
    uploadedBytes: snapshot.uploadedBytes,
    uploadSpeedBytesPerSecond: snapshot.uploadSpeedBytesPerSecond,
    peerCount: snapshot.peerCount,
    uploadStatus: snapshot.uploadStatus,
    infoHash: snapshot.infoHash,
    error: snapshot.error,
    cdnAccelerated: snapshot.cdnAccelerated,
    createdAtMs: snapshot.createdAtMs,
    seedCount: snapshot.seedCount,
    leechCount: snapshot.leechCount,
    downloadLimitBps: snapshot.downloadLimitBps,
    uploadLimitBps: snapshot.uploadLimitBps,
  };
}
