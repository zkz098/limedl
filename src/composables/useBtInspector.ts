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

  // Per-tab version counters to reject stale concurrent callbacks.
  // Without this, overlapping calls (polling + manual refresh) can
  // overwrite each other with stale / empty data on a last-write-wins basis.
  const version = reactive({ files: 0, peers: 0, trackers: 0, pieces: 0 });

  async function fetchTabData<T>(
    name: keyof typeof isLoading,
    fetcher: (id: string) => Promise<T>,
    onSuccess: (data: T) => void,
  ) {
    if (!taskId.value) return;
    const ver = ++version[name];
    isLoading[name] = true;
    errors[name] = "";
    const id = taskId.value;
    try {
      const data = await fetcher(id);
      // Reject if either: the taskId changed, or a newer call of the same kind has started.
      if (version[name] === ver && taskId.value === id) onSuccess(data);
    } catch (e) {
      if (version[name] === ver && taskId.value === id) {
        errors[name] = String(e);
        console.error(`[useBtInspector] ${name} fetch failed:`, e);
      }
    } finally {
      if (version[name] === ver && taskId.value === id) isLoading[name] = false;
    }
  }

  async function fetchFiles() {
    await fetchTabData("files", getBtFiles, (data) => {
      files.value = data;
    });
  }

  async function fetchPeers() {
    await fetchTabData("peers", getBtPeers, (data) => {
      peers.value = data;
    });
  }

  async function fetchTrackers() {
    await fetchTabData("trackers", getBtTrackers, (data) => {
      trackers.value = data;
    });
  }

  async function fetchPieces() {
    await fetchTabData("pieces", getBtPieces, (data) => {
      pieces.value = data;
    });
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
