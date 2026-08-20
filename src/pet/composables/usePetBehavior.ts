import { ref, onMounted, onUnmounted } from "vue";
import { listen } from "#event";

export type PetState = "idle" | "work" | "celebrate" | "sad" | "drag" | "sleep";

const IDLE_TIMEOUT_MS = 30_000;
const CELEBRATE_MS = 3000;
const SAD_MS = 2000;

export function usePetBehavior() {
  const state = ref<PetState>("idle");
  let idleTimer: number | null = null;
  let revertTimer: number | null = null;
  let unlistenProgress: (() => void) | null = null;
  let unlistenUpdated: (() => void) | null = null;
  let unlistenWarning: (() => void) | null = null;
  let activeDownloadCount = 0;

  function setState(next: PetState, autoRevertMs?: number) {
    if (state.value === "drag" && next !== "drag") {
      // Don't override drag unless explicitly leaving drag
      if (next !== "idle") return;
    }
    state.value = next;
    if (revertTimer !== null) {
      window.clearTimeout(revertTimer);
      revertTimer = null;
    }
    if (autoRevertMs) {
      revertTimer = window.setTimeout(() => {
        state.value = activeDownloadCount > 0 ? "work" : "idle";
        resetIdleTimer();
      }, autoRevertMs);
    }
    if (next === "idle" || next === "work") {
      resetIdleTimer();
    }
  }

  function resetIdleTimer() {
    if (idleTimer !== null) window.clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => {
      if (state.value === "idle") setState("sleep");
    }, IDLE_TIMEOUT_MS);
  }

  function onDragStart() {
    if (revertTimer !== null) {
      window.clearTimeout(revertTimer);
      revertTimer = null;
    }
    state.value = "drag";
  }

  function onDragEnd() {
    state.value = activeDownloadCount > 0 ? "work" : "idle";
    resetIdleTimer();
  }

  function onDropSuccess() {
    setState("celebrate", CELEBRATE_MS);
  }

  async function setupListeners() {
    try {
      // download-progress: contains speed/state
      const un1 = await listen<{ state?: string; speed_bytes_per_second?: number | null }>(
        "download-progress",
        (event) => {
          const payload = event.payload;
          // Heuristic: if any download is Downloading, show work
          if (payload.state === "downloading") {
            activeDownloadCount = 1;
            if (state.value !== "drag" && state.value !== "celebrate" && state.value !== "sad") {
              setState("work");
            }
          } else if (payload.state === "completed") {
            setState("celebrate", CELEBRATE_MS);
            activeDownloadCount = 0;
          } else if (payload.state === "failed") {
            setState("sad", SAD_MS);
          }
        },
      );
      unlistenProgress = un1;

      const un2 = await listen<{ state?: string }>("download-updated", (event) => {
        const payload = event.payload;
        if (payload.state === "completed") {
          setState("celebrate", CELEBRATE_MS);
        } else if (payload.state === "failed") {
          setState("sad", SAD_MS);
        }
      });
      unlistenUpdated = un2;

      const un3 = await listen("download-warning", () => {
        setState("sad", SAD_MS);
      });
      unlistenWarning = un3;
    } catch (e) {
      console.warn("[pet] event listen failed (maybe NAS mode)", e);
    }
  }

  onMounted(() => {
    resetIdleTimer();
    void setupListeners();
  });

  onUnmounted(() => {
    if (idleTimer !== null) window.clearTimeout(idleTimer);
    if (revertTimer !== null) window.clearTimeout(revertTimer);
    unlistenProgress?.();
    unlistenUpdated?.();
    unlistenWarning?.();
  });

  return {
    state,
    onDragStart,
    onDragEnd,
    onDropSuccess,
    setState,
  };
}
