import { t } from "../i18n";
import type { DownloadSnapshot, DownloadState, DownloadSummary } from "../types/download";

export const terminalStates: DownloadState[] = ["completed", "failed", "canceled"];

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
  [/permission denied/i, "errors.permissionDenied"],
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
  // Destructure only the fields that DownloadSummary expects.
  // TypeScript enforces that every non-optional Summary field is listed.
  // If a new field is added to DownloadSummary, this line will error,
  // forcing a deliberate decision about whether to include it.
  const {
    id,
    kind,
    state,
    url,
    fileName,
    destinationPath,
    totalBytes,
    downloadedBytes,
    connectionCount,
    threadMode,
    requestedThreadCount,
    desiredThreadCount,
    allocatedThreadCount,
    adaptiveProfile,
    threadNote,
    speedBytesPerSecond,
    etaSeconds,
    uploadedBytes,
    uploadSpeedBytesPerSecond,
    peerCount,
    uploadStatus,
    infoHash,
    error,
    cdnAccelerated,
    degraded,
    diskType,
    flushing,
    createdAtMs,
    seedCount,
    leechCount,
    downloadLimitBps,
    uploadLimitBps,
  } = snapshot;
  return {
    id,
    kind,
    state,
    url,
    fileName,
    destinationPath,
    totalBytes,
    downloadedBytes,
    connectionCount,
    threadMode,
    requestedThreadCount,
    desiredThreadCount,
    allocatedThreadCount,
    adaptiveProfile,
    threadNote,
    speedBytesPerSecond,
    etaSeconds,
    uploadedBytes,
    uploadSpeedBytesPerSecond,
    peerCount,
    uploadStatus,
    infoHash,
    error,
    cdnAccelerated,
    degraded,
    diskType,
    flushing,
    createdAtMs,
    seedCount,
    leechCount,
    downloadLimitBps,
    uploadLimitBps,
  };
}

export function toneForState(state: string): "info" | "success" | "warning" | "danger" {
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
}
