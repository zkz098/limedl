import { onMounted, ref } from "vue";
import { getOverclockMode, toggleOverclockMode } from "../lib/tauri/settings-api";

const overclockMode = ref(false);

export function useOverclock() {
  onMounted(() => {
    void fetchOverclockMode();
  });
  async function fetchOverclockMode() {
    try {
      overclockMode.value = await getOverclockMode();
    } catch (error) {
      console.error("[useOverclock] Failed to fetch overclock mode:", error);
    }
  }

  async function setOverclockMode(enabled: boolean) {
    try {
      await toggleOverclockMode(enabled);
      overclockMode.value = enabled;
    } catch (error) {
      console.error("[useOverclock] Failed to toggle overclock mode:", error);
      throw error;
    }
  }

  return {
    overclockMode,
    fetchOverclockMode,
    setOverclockMode,
  };
}
