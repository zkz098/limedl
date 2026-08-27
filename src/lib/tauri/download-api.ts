import { invoke } from "#invoke";
import { commandName } from "../ws/command-name";

import type {
  BtFileStatus,
  BtPeerInfo,
  BtPieceInfo,
  BtRuntimeStatus,
  BtTrackerInfo,
  DownloadSnapshot,
  DownloadSummary,
  Priority,
  StartDownloadRequest,
  TorrentFileEntry,
} from "../../types/download";

export interface TaskIdResult {
  kind: "http" | "bt";
  id: string;
}

export function startDownload(request: StartDownloadRequest) {
  return invoke<TaskIdResult>(commandName("download_start"), { request });
}

export function pauseDownload(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_pause"), { downloadId });
}

export function resumeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_resume"), { downloadId });
}

export function cancelDownload(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_cancel"), { downloadId });
}

export function removeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_remove"), { downloadId });
}

export function purgeDownload(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_purge"), { downloadId });
}

export function openDownloadInExplorer(downloadId: string) {
  return invoke<void>(commandName("download_open_in_explorer"), { downloadId });
}

export function openDownloadFile(downloadId: string) {
  return invoke<void>(commandName("download_open_file"), { downloadId });
}

export function openDownloadDir(downloadId: string) {
  return invoke<void>(commandName("download_open_dir"), { downloadId });
}

export function getDownloadStatus(downloadId: string) {
  return invoke<DownloadSnapshot>(commandName("download_status"), { downloadId });
}

export function listDownloads() {
  return invoke<DownloadSummary[]>(commandName("download_list"));
}

export function getBtRuntimeStatus() {
  return invoke<BtRuntimeStatus>(commandName("bt_runtime_status"));
}

export function getBtPeers(downloadId: string) {
  return invoke<BtPeerInfo[]>(commandName("bt_get_peers"), { downloadId });
}

export function getBtTrackers(downloadId: string) {
  return invoke<BtTrackerInfo[]>(commandName("bt_get_trackers"), { downloadId });
}

export function getBtPieces(downloadId: string) {
  return invoke<BtPieceInfo[]>(commandName("bt_get_pieces"), { downloadId });
}

export function setBtSpeedLimit(
  downloadId: string,
  downloadLimitBps?: number,
  uploadLimitBps?: number,
) {
  return invoke<void>(commandName("bt_set_speed_limit"), {
    downloadId,
    downloadLimitBps,
    uploadLimitBps,
  });
}

export function setPriority(downloadId: string, priority: Priority) {
  return invoke<void>(commandName("download_set_priority"), { downloadId, priority });
}

export function previewTorrent(source: string) {
  return invoke<TorrentFileEntry[]>(commandName("bt_preview_torrent"), { source });
}

export function getBtFiles(downloadId: string) {
  return invoke<BtFileStatus[]>(commandName("get_bt_files"), { downloadId });
}

export function updateBtFiles(downloadId: string, includedIndices: number[]) {
  return invoke<void>(commandName("update_bt_files"), { downloadId, includedIndices });
}
