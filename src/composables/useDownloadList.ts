import { ref, type Ref } from "vue";

import { listDownloads } from "../lib/tauri/download-api";
import { t } from "../i18n";
import { toMessage } from "./downloadHelpers";
import type { DownloadSnapshot, DownloadSummary } from "../types/download";

export interface UseDownloadListInput {
  downloads: Ref<DownloadSummary[]>;
  selectedId: Ref<string | null>;
  selectedSnapshot: Ref<DownloadSnapshot | null>;
  allowAutoSelect: Ref<boolean>;
  isAutoRefreshing: Ref<boolean>;
  ensureSelection: () => void;
  setMessage: (message: string) => void;
  setError: (message: string) => void;
}

export function useDownloadList(input: UseDownloadListInput) {
  const {
    downloads,
    selectedId,
    selectedSnapshot,
    isAutoRefreshing,
    ensureSelection,
    setMessage,
    setError,
  } = input;

  const isRefreshingList = ref(false);

  async function refreshList(options?: { silent?: boolean }) {
    if (isRefreshingList.value) {
      return;
    }

    isRefreshingList.value = true;

    try {
      downloads.value = await listDownloads();
      ensureSelection();

      if (
        selectedId.value &&
        selectedSnapshot.value &&
        selectedSnapshot.value.id !== selectedId.value
      ) {
        selectedSnapshot.value = null;
      }

      if (!downloads.value.length && !options?.silent) {
        setMessage(t("messages.noDownloads"));
      } else if (!options?.silent && downloads.value.length) {
        setMessage(t("messages.queueRefreshed", { count: downloads.value.length }));
      }
    } catch (error) {
      if (!options?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingList.value = false;
    }
  }

  return {
    downloads,
    isRefreshingList,
    isAutoRefreshing,
    refreshList,
  };
}
