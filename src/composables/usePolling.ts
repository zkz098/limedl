import { onUnmounted, ref, type Ref } from "vue";

/**
 * Polling composable for auto-refresh.
 * Automatically cleans up intervals on component unmount.
 * 
 * Usage:
 * const { isPolling, start, stop } = usePolling(async () => {
 *   await refreshList();
 * }, 1500); // 1.5 second interval
 */
export function usePolling(
  callback: () => Promise<void>,
  interval: number = 2000
): {
  isPolling: Ref<boolean>;
  start: () => void;
  stop: () => void;
} {
  const isPolling = ref(false);
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function poll() {
    try {
      await callback();
    } catch (error) {
      console.error("[usePolling] Error during polling:", error);
    }
  }

  function start() {
    if (isPolling.value) {
      console.warn("[usePolling] Polling already running");
      return;
    }

    isPolling.value = true;
    
    // Execute immediately on start
    poll().catch(console.error);
    
    // Then set interval for subsequent calls
    intervalId = setInterval(() => {
      poll().catch(console.error);
    }, interval);
  }

  function stop() {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
    isPolling.value = false;
  }

  // Auto-cleanup on component unmount
  onUnmounted(() => {
    stop();
  });

  return {
    isPolling,
    start,
    stop,
  };
}
