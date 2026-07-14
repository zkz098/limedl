import { computed, onMounted, onUnmounted, ref } from "vue";

import { getIoStatus, toggleGameMode, type IoStatus } from "../lib/tauri/settings-api";

export type { IoStatus };

const status = ref<IoStatus | null>(null);
const isToggling = ref(false);
let pollConsumers = 0;
let intervalId: ReturnType<typeof setInterval> | null = null;

async function fetchStatus() {
  try {
    status.value = await getIoStatus();
  } catch (error) {
    console.error("[useIoBaseline] Failed to fetch I/O status:", error);
  }
}

function startPolling() {
  pollConsumers += 1;
  if (pollConsumers > 1) {
    return;
  }

  void fetchStatus();
  intervalId = setInterval(() => {
    void fetchStatus();
  }, 2000);
}

function stopPolling() {
  pollConsumers = Math.max(0, pollConsumers - 1);
  if (pollConsumers === 0 && intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
}

export function useIoBaseline() {
  onMounted(() => {
    startPolling();
  });

  onUnmounted(() => {
    stopPolling();
  });

  const gameMode = computed(() => status.value?.gameMode ?? false);
  const bufferUsageBytes = computed(() => status.value?.bufferUsageBytes ?? 0);
  const bufferLimitBytes = computed(() => status.value?.bufferLimitBytes ?? 0);
  const degradationCount = computed(() => status.value?.degradationCount ?? 0);

  async function setGameMode(enabled: boolean) {
    if (isToggling.value) {
      return;
    }

    isToggling.value = true;

    try {
      await toggleGameMode(enabled);
      status.value = await getIoStatus();
    } catch (error) {
      console.error("[useIoBaseline] Failed to toggle game mode:", error);
      throw error;
    } finally {
      isToggling.value = false;
    }
  }

  async function refreshStatus() {
    await fetchStatus();
  }

  return {
    status,
    gameMode,
    bufferUsageBytes,
    bufferLimitBytes,
    degradationCount,
    isToggling,
    setGameMode,
    refreshStatus,
  };
}
