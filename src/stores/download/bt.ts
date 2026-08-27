import { ref } from "vue";
import { defineStore } from "pinia";
import { getBtRuntimeStatus, getDownloadStatus } from "../../lib/tauri/download-api";
import { t } from "../../i18n";
import { toMessage, toSummary } from "../../composables/downloadHelpers";
import { useNotificationStore } from "../notification";
import type { BtRuntimeStatus, DownloadSnapshot, DownloadSummary } from "../../types/download";

export const useBtDetailStore = defineStore("btDetail", () => {
  const notify = useNotificationStore();

  const btRuntimeStatus = ref<BtRuntimeStatus | null>(null);
  const isRefreshingStatus = ref(false);

  function setMessage(message: string) {
    notify.notifyInfo(message);
  }

  function setError(message: string) {
    notify.notifyError(message);
  }

  async function refreshBtRuntimeStatus(opts?: { silent?: boolean }) {
    try {
      btRuntimeStatus.value = await getBtRuntimeStatus();
    } catch (error) {
      if (!opts?.silent) {
        setError(toMessage(error));
      }
    }
  }

  async function refreshStatus(
    downloadId: string | null,
    onSnapshot: (snapshot: DownloadSnapshot, summary: DownloadSummary) => void,
    opts?: { silent?: boolean },
  ) {
    if (!downloadId) return;
    if (isRefreshingStatus.value) return;

    isRefreshingStatus.value = true;

    try {
      const snapshot = await getDownloadStatus(downloadId);
      const summary = toSummary(snapshot);
      onSnapshot(snapshot, summary);

      if (!opts?.silent) {
        setMessage(t("messages.statusRefreshed", { fileName: snapshot.fileName }));
      }
    } catch (error) {
      if (!opts?.silent) {
        setError(toMessage(error));
      }
    } finally {
      isRefreshingStatus.value = false;
    }
  }

  return {
    btRuntimeStatus,
    isRefreshingStatus,
    refreshBtRuntimeStatus,
    refreshStatus,
  };
});
