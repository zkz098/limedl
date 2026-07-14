import { invoke } from "@tauri-apps/api/core";

import type {
  BtFileStatus,
  BtPeerInfo,
  BtPieceInfo,
  BtRuntimeStatus,
  BtTrackerInfo,
  DownloadSnapshot,
  DownloadSummary,
  StartDownloadRequest,
  TorrentFileEntry,
} from "../../types/download";

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

export function getBtRuntimeStatus() {
  return invoke<BtRuntimeStatus>("bt_runtime_status");
}

export function getBtPeers(downloadId: string) {
  return invoke<BtPeerInfo[]>("bt_get_peers", { downloadId });
}

export function getBtTrackers(downloadId: string) {
  return invoke<BtTrackerInfo[]>("bt_get_trackers", { downloadId });
}

export function getBtPieces(downloadId: string) {
  return invoke<BtPieceInfo[]>("bt_get_pieces", { downloadId });
}

export function setBtSpeedLimit(
  downloadId: string,
  downloadLimitBps?: number,
  uploadLimitBps?: number,
) {
  return invoke<void>("bt_set_speed_limit", { downloadId, downloadLimitBps, uploadLimitBps });
}

export function previewTorrent(source: string) {
  return invoke<TorrentFileEntry[]>("bt_preview_torrent", { source });
}

export function getBtFiles(downloadId: string) {
  return invoke<BtFileStatus[]>("get_bt_files", { downloadId });
}

export function updateBtFiles(downloadId: string, includedIndices: number[]) {
  return invoke<void>("update_bt_files", { downloadId, includedIndices });
}
