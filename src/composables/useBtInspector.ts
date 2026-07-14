import { reactive, ref, watch, type Ref } from "vue";

import {
  getBtFiles,
  getBtPeers,
  getBtTrackers,
  getBtPieces,
  updateBtFiles,
} from "../lib/tauri/download-api";
import type { BtFileStatus, BtPeerInfo, BtPieceInfo, BtTrackerInfo } from "../types/download";

export function useBtInspector(taskId: Ref<string | null>) {
  const files = ref<BtFileStatus[]>([]);
  const peers = ref<BtPeerInfo[]>([]);
  const trackers = ref<BtTrackerInfo[]>([]);
  const pieces = ref<BtPieceInfo[]>([]);

  const isLoading = reactive({
    files: false,
    peers: false,
    trackers: false,
    pieces: false,
  });
  const errors = reactive({
    files: "",
    peers: "",
    trackers: "",
    pieces: "",
  });

  const isUpdatingFiles = ref(false);

  /** AbortController to cancel in-flight fetches on taskId change. */
  let abortController: AbortController | null = null;

  async function fetchFiles() {
    const id = taskId.value;
    if (!id) return;
    isLoading.files = true;
    errors.files = "";
    try {
      const result = await getBtFiles(id);
      if (taskId.value === id) {
        files.value = result;
      }
    } catch {
      if (taskId.value === id) {
        files.value = [];
        errors.files = "Failed to fetch file list";
      }
    } finally {
      if (taskId.value === id) {
        isLoading.files = false;
      }
    }
  }

  async function fetchPeers() {
    const id = taskId.value;
    if (!id) return;
    isLoading.peers = true;
    errors.peers = "";
    try {
      const result = await getBtPeers(id);
      if (taskId.value === id) {
        peers.value = result;
      }
    } catch {
      if (taskId.value === id) {
        peers.value = [];
        errors.peers = "Failed to fetch peers";
      }
    } finally {
      if (taskId.value === id) {
        isLoading.peers = false;
      }
    }
  }

  async function fetchTrackers() {
    const id = taskId.value;
    if (!id) return;
    isLoading.trackers = true;
    errors.trackers = "";
    try {
      const result = await getBtTrackers(id);
      if (taskId.value === id) {
        trackers.value = result;
      }
    } catch {
      if (taskId.value === id) {
        trackers.value = [];
        errors.trackers = "Failed to fetch trackers";
      }
    } finally {
      if (taskId.value === id) {
        isLoading.trackers = false;
      }
    }
  }

  async function fetchPieces() {
    const id = taskId.value;
    if (!id) return;
    isLoading.pieces = true;
    errors.pieces = "";
    try {
      const result = await getBtPieces(id);
      if (taskId.value === id) {
        pieces.value = result;
      }
    } catch {
      if (taskId.value === id) {
        pieces.value = [];
        errors.pieces = "Failed to fetch pieces";
      }
    } finally {
      if (taskId.value === id) {
        isLoading.pieces = false;
      }
    }
  }

  async function toggleFileInclusion(fileIndex: number, currentlyIncluded: boolean) {
    const newIncluded = new Set(files.value.filter((f) => f.included).map((f) => f.index));
    if (currentlyIncluded) {
      newIncluded.delete(fileIndex);
    } else {
      newIncluded.add(fileIndex);
    }
    // Prevent deselecting all files — at least one must remain
    if (newIncluded.size === 0) return;

    const id = taskId.value;
    if (!id) return;

    isUpdatingFiles.value = true;
    try {
      await updateBtFiles(id, [...newIncluded]);
      // Optimistic local update
      const file = files.value.find((f) => f.index === fileIndex);
      if (file) file.included = !currentlyIncluded;
    } catch {
      errors.files = "Failed to update file inclusion";
      // Revert on error — refetch
      if (taskId.value) {
        await fetchFiles();
      }
    } finally {
      isUpdatingFiles.value = false;
    }
  }

  function clear() {
    files.value = [];
    peers.value = [];
    trackers.value = [];
    pieces.value = [];
  }

  watch(
    taskId,
    (id) => {
      // Abort any in-flight requests before starting new ones
      abortController?.abort();
      abortController = new AbortController();

      if (id) {
        void Promise.all([fetchPeers(), fetchTrackers(), fetchPieces()]);
      } else {
        clear();
      }
    },
    { immediate: true },
  );

  return {
    files,
    peers,
    trackers,
    pieces,
    isLoading,
    errors,
    isUpdatingFiles,
    fetchFiles,
    fetchPeers,
    fetchTrackers,
    fetchPieces,
    toggleFileInclusion,
    clear,
  };
}
