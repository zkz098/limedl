import type { DownloadState, DownloadSummary } from "../types/download";
import { t } from "../i18n";

type ProgressShape = Pick<DownloadSummary, "downloadedBytes" | "totalBytes" | "state">;

export function formatTokenLabel(value?: string) {
  if (!value) {
    return t("common.unknown");
  }

  return t(`tokens.${value}`);
}

export function formatBytes(value?: number) {
  if (typeof value !== "number") {
    return "—";
  }

  if (value === 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let index = 0;

  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }

  const precision = size >= 100 || index === 0 ? 0 : 1;
  return `${size.toFixed(precision)} ${units[index]}`;
}

export function formatSpeed(value?: number) {
  if (typeof value !== "number") {
    return "—";
  }

  return `${formatBytes(value)}/s`;
}

export function formatEta(value?: number) {
  if (typeof value !== "number") {
    return "—";
  }

  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const seconds = value % 60;
  const parts = [hours ? `${hours}h` : "", minutes ? `${minutes}m` : "", `${seconds}s`].filter(
    Boolean,
  );

  return parts.join(" ");
}

export function formatTimestamp(value?: number) {
  if (typeof value !== "number") {
    return "—";
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(value);
}

export function stateLabel(state?: DownloadState) {
  return state ? t(`states.${state}`) : formatTokenLabel(state);
}

export function isSizeUnknown(download: ProgressShape) {
  return !download.totalBytes || download.totalBytes <= 0;
}

export function progressValue(download: ProgressShape) {
  const total = download.totalBytes;
  if (!total || total <= 0) {
    return download.state === "completed" ? 100 : 0;
  }

  return Math.min((download.downloadedBytes / total) * 100, 100);
}

export function progressLabel(download: ProgressShape) {
  if (!download.totalBytes || download.totalBytes <= 0) {
    return download.state === "completed" ? "100%" : t("queue.pendingSize");
  }

  return `${progressValue(download).toFixed(1)}%`;
}
