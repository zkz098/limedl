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

function snakeToCamel(str: string): string {
  return str.replace(/_([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}

export function toFriendlyError(raw: string, kind?: string | null): string {
  if (kind) {
    const camelKind = snakeToCamel(kind);
    const key = `errors.${camelKind}`;
    const translated = t(key);
    if (translated !== key) {
      return translated;
    }
  }

  // Check if raw message contains a [kind] prefix (e.g. "[insufficient_disk_space] ...")
  const prefixMatch = raw.match(/^\[([a-z0-9_]+)\]/i);
  if (prefixMatch && prefixMatch[1]) {
    const camelKind = snakeToCamel(prefixMatch[1]);
    const key = `errors.${camelKind}`;
    const translated = t(key);
    if (translated !== key) {
      return translated;
    }
  }

  for (const [pattern, key] of errorPatterns) {
    if (pattern.test(raw)) {
      return t(key);
    }
  }
  return raw;
}

/**
 * Extract a human-readable message from any error value.
 * Handles `Error` instances as well as plain objects rejected by
 * Tauri's IPC layer (e.g. `SerializableError` shaped `{ kind, message }`).
 */
export function toErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null) {
    const maybe = (error as { message?: unknown }).message;
    if (typeof maybe === "string" && maybe.length > 0) return maybe;
  }
  return String(error);
}

export function toMessage(error: unknown): string {
  let kind: string | undefined;
  if (typeof error === "object" && error !== null) {
    const maybeKind = (error as { kind?: unknown }).kind;
    if (typeof maybeKind === "string" && maybeKind.length > 0) {
      kind = maybeKind;
    }
  }
  return toFriendlyError(toErrorMessage(error), kind);
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
    priority,
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
    diskType: diskType ?? undefined,
    flushing,
    createdAtMs,
    seedCount,
    leechCount,
    downloadLimitBps,
    uploadLimitBps,
    priority,
  };
}

export function toneForState(state: string): "info" | "success" | "warning" | "danger" {
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
}
