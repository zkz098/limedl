import { ref, type Ref } from "vue";

export function useMultiSelect(downloads: Ref<Array<{ id: string }>>) {
  const multiSelectMode = ref(false);
  const selectedIds = ref(new Set<string>());
  const showBatchDeleteDialog = ref(false);
  const removedDownloadIds = ref<string[]>([]);

  function handleToggleMultiSelectMode(enabled: boolean) {
    multiSelectMode.value = enabled;
    if (!enabled) {
      selectedIds.value = new Set();
    }
  }

  function handleToggleSelect(downloadId: string) {
    const next = new Set(selectedIds.value);
    if (next.has(downloadId)) {
      next.delete(downloadId);
      if (next.size === 0) {
        multiSelectMode.value = false;
      }
    } else {
      next.add(downloadId);
    }
    selectedIds.value = next;
  }

  function handleSelectAll() {
    selectedIds.value = new Set(downloads.value.map((d) => d.id));
  }

  function handleDeselectAll() {
    selectedIds.value = new Set();
    multiSelectMode.value = false;
  }

  function handleBatchDelete() {
    if (selectedIds.value.size === 0) return;
    showBatchDeleteDialog.value = true;
  }

  return {
    multiSelectMode,
    selectedIds,
    showBatchDeleteDialog,
    removedDownloadIds,
    handleToggleMultiSelectMode,
    handleToggleSelect,
    handleSelectAll,
    handleDeselectAll,
    handleBatchDelete,
  };
}
