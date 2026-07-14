import { t } from "../i18n";
import type { DownloadSnapshot, DownloadState, DownloadSummary } from "../types/download";

export const terminalStates: DownloadState[] = ["completed", "failed", "canceled"];
export const autoRefreshIntervalMs = 1500;

export function canPauseState(state?: DownloadState | null) {
  return Boolean(state && ["queued", "downloading", "retrying", "verifying"].includes(state));
}

export function canResumeState(state?: DownloadState | null) {
  return state === "paused" || state === "failed";
}

const errorPatterns: [RegExp, string][] = [
  [/http status 401/i, "errors.http401"],
  [/http status 403/i, "errors.http403"],
  [/http status 404/i, "errors.http404"],
  [/http status 5\d{2}/i, "errors.http5xx"],
  [/certificate verify failed/i, "errors.sslCert"],
  [/tls_process|handshake|no protocols/i, "errors.sslHandshake"],
  [/connection refused/i, "errors.connectionRefused"],
  [/timed out|timeout/i, "errors.connectionTimeout"],
  [/dns|name resolution/i, "errors.dnsFailure"],
  [/insufficient disk space|disk space/i, "errors.insufficientDiskSpace"],
  [/no route to host|network is unreachable|network error/i, "errors.networkError"],
  [/internal server error|server error/i, "errors.serverError"],
];

export function toFriendlyError(raw: string): string {
  for (const [pattern, key] of errorPatterns) {
    if (pattern.test(raw)) {
      return t(key);
    }
  }
  return raw;
}

export function toMessage(error: unknown) {
  let message: string;

  if (error instanceof Error) {
    message = error.message;
  } else {
    message = String(error);
  }

  return toFriendlyError(message);
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
