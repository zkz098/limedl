import { invoke } from "@tauri-apps/api/core";

import type { DownloadSnapshot, DownloadSummary, StartDownloadRequest } from "../../types/download";

export function startDownload(request: StartDownloadRequest) {
  return invoke<string>("download_start", { request });
}

export function pauseDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_pause", { downloadId });
}

export function resumeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_resume", { downloadId });
}

export function cancelDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_cancel", { downloadId });
}

export function removeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_remove", { downloadId });
}

export function purgeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_purge", { downloadId });
}

export function openDownloadInExplorer(downloadId: string) {
  return invoke<void>("download_open_in_explorer", { downloadId });
}

export function getDownloadStatus(downloadId: string) {
  return invoke<DownloadSnapshot>("download_status", { downloadId });
}

export function listDownloads() {
  return invoke<DownloadSummary[]>("download_list");
}
