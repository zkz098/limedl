import { invoke } from "@tauri-apps/api/core";

import type { DownloadSnapshot, DownloadSummary, StartDownloadRequest } from "../../types/download";

export function startDownload(request: StartDownloadRequest) {
  return invoke<string>("download_start", { request });
}

export function pauseDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_pause", { download_id: downloadId });
}

export function resumeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_resume", { download_id: downloadId });
}

export function cancelDownload(downloadId: string) {
  return invoke<DownloadSnapshot>("download_cancel", { download_id: downloadId });
}

export function getDownloadStatus(downloadId: string) {
  return invoke<DownloadSnapshot>("download_status", { download_id: downloadId });
}

export function listDownloads() {
  return invoke<DownloadSummary[]>("download_list");
}
