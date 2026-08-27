import {
  isNotificationPermissionGranted,
  requestNotificationPermission,
  sendNotification,
} from "../../lib/platform";
import { canPauseState, canResumeState } from "../../composables/downloadHelpers";
import type { DownloadProgress, DownloadSnapshot, DownloadSummary } from "../../types/download";

// ── OS notification (standalone, not part of store) ────────────────

export async function fireNotification(
  title: string,
  body: string,
  downloadId?: string,
): Promise<void> {
  try {
    let granted = await isNotificationPermissionGranted();
    if (!granted) {
      const permission = await requestNotificationPermission();
      granted = permission === "granted";
    }
    if (granted) {
      await sendNotification({
        title,
        body,
        ...(downloadId ? { extra: { downloadId } } : {}),
      });
    }
  } catch {
    // Silently fail — notifications are non-critical
  }
}

// ── URL range expansion ────────────────────────────────────────────

/** Expand URL range patterns like file[01-20].zip or file[1-20].zip */
export function expandUrlRanges(url: string): string[] {
  const rangeRegex = /\[(\d+)-(\d+)\]/;
  const match = rangeRegex.exec(url);
  if (!match) return [url];
  const start = Number.parseInt(match[1], 10);
  const end = Number.parseInt(match[2], 10);
  if (start > end) return [url];
  const padding = match[1].length;
  const results: string[] = [];
  for (let i = start; i <= end; i++) {
    results.push(url.replace(rangeRegex, String(i).padStart(padding, "0")));
  }
  return results;
}

export function canPauseDownload(download: DownloadSummary): boolean {
  return canPauseState(download.state);
}

export function canResumeDownload(download: DownloadSummary): boolean {
  return canResumeState(download.state);
}

/** Copy the always-present progress fields and the optional non-null ones onto a task summary. */
export function applyProgressToSummary(
  existing: DownloadSummary,
  progress: DownloadProgress,
): void {
  existing.state = progress.state;
  existing.downloadedBytes = progress.downloadedBytes;
  existing.connectionCount = progress.connectionCount;
  if (progress.totalBytes != null) existing.totalBytes = progress.totalBytes;
  if (progress.speedBytesPerSecond != null)
    existing.speedBytesPerSecond = progress.speedBytesPerSecond;
  if (progress.etaSeconds != null) existing.etaSeconds = progress.etaSeconds;
  if (progress.allocatedThreadCount != null)
    existing.allocatedThreadCount = progress.allocatedThreadCount;
  if (progress.error != null) existing.error = progress.error;
  if (progress.uploadedBytes != null) existing.uploadedBytes = progress.uploadedBytes;
  if (progress.uploadSpeedBytesPerSecond != null)
    existing.uploadSpeedBytesPerSecond = progress.uploadSpeedBytesPerSecond;
  if (progress.peerCount != null) existing.peerCount = progress.peerCount;
  if (progress.uploadStatus != null) existing.uploadStatus = progress.uploadStatus;
  if (progress.degraded != null) existing.degraded = progress.degraded;
  if (progress.diskType != null) existing.diskType = progress.diskType;
  if (progress.flushing != null) existing.flushing = progress.flushing;
}

/** Mirror the live progress onto the detail side-panel snapshot (when selected). */
export function applyProgressToSnapshot(
  snapshot: DownloadSnapshot,
  progress: DownloadProgress,
): void {
  Object.assign(snapshot, {
    downloadedBytes: progress.downloadedBytes,
    state: progress.state,
    ...(progress.totalBytes != null && { totalBytes: progress.totalBytes }),
    ...(progress.speedBytesPerSecond != null && {
      speedBytesPerSecond: progress.speedBytesPerSecond,
    }),
    ...(progress.etaSeconds != null && { etaSeconds: progress.etaSeconds }),
    ...(progress.connectionCount !== undefined && {
      connectionCount: progress.connectionCount,
    }),
    ...(progress.error != null && { error: progress.error }),
    ...(progress.uploadedBytes != null && { uploadedBytes: progress.uploadedBytes }),
    ...(progress.uploadSpeedBytesPerSecond != null && {
      uploadSpeedBytesPerSecond: progress.uploadSpeedBytesPerSecond,
    }),
    ...(progress.peerCount != null && { peerCount: progress.peerCount }),
    ...(progress.uploadStatus != null && { uploadStatus: progress.uploadStatus }),
    ...(progress.degraded != null && { degraded: progress.degraded }),
    ...(progress.diskType != null && { diskType: progress.diskType }),
    ...(progress.flushing != null && { flushing: progress.flushing }),
  });
}
