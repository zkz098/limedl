import { ref, type Ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";

/**
 * Tauri file dialog composable.
 * Opens a native directory picker and returns the selected path.
 *
 * Usage:
 * const { selectedPath, isOpen, pick } = useFileDialog();
 * await pick(); // Opens dialog
 * console.log(selectedPath.value); // Selected directory path
 */
export function useFileDialog(): {
  selectedPath: Ref<string | null>;
  isOpen: Ref<boolean>;
  pick: () => Promise<void>;
  clear: () => void;
} {
  const selectedPath = ref<string | null>(null);
  const isOpen = ref(false);

  async function pick() {
    try {
      isOpen.value = true;

      const result = await open({
        directory: true,
        multiple: false,
        title: "Select download directory",
      });

      if (result) {
        selectedPath.value = result as string;
      }
    } catch (error) {
      console.error("[useFileDialog] Error opening file picker:", error);
      throw error;
    } finally {
      isOpen.value = false;
    }
  }

  function clear() {
    selectedPath.value = null;
  }

  return {
    selectedPath,
    isOpen,
    pick,
    clear,
  };
}
