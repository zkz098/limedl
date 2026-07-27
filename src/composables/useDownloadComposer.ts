/**
 * Download composer composable.
 *
 * Provides form state and actions for the download composer dialog.
 * Uses a module-level singleton pattern so all callers share the same state.
 *
 * In a future refactor, this state would be extracted out of the Pinia store
 * entirely. For now it wraps the store's form-related getters and actions.
 */
import { useDownloadStore } from "../stores/download";
import { storeToRefs } from "pinia";

let sharedState: ReturnType<typeof createState> | null = null;

function createState() {
  const downloadStore = useDownloadStore();

  const {
    form,
    isPickingDirectory,
    isPickingTorrent,
    isStarting,
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,
  } = storeToRefs(downloadStore);

  return {
    // Refs
    form,
    isPickingDirectory,
    isPickingTorrent,
    isStarting,
    batchMode,
    batchUrls,
    batchEntries,
    batchSubmitProgress,

    // Actions
    pickDestinationDirectory: downloadStore.pickDestinationDirectory,
    pickTorrentSourceFile: downloadStore.pickTorrentSourceFile,
    submitStart: downloadStore.submitStart,
    autoFillFromClipboard: downloadStore.autoFillFromClipboard,
    parseBatchUrls: downloadStore.parseBatchUrls,
    submitBatch: downloadStore.submitBatch,
    toggleBatchMode: downloadStore.toggleBatchMode,
    setMessage: downloadStore.setMessage,
    setError: downloadStore.setError,
  };
}

export function useDownloadComposer() {
  if (!sharedState) {
    sharedState = createState();
  }
  return sharedState;
}
