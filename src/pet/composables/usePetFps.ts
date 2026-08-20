import { ref, watch, onMounted, onUnmounted } from "vue";
import type { PetState } from "./usePetBehavior";

function getFps(s: PetState): number {
  if (s === "celebrate" || s === "drag") return 24;
  if (s === "sleep") return 6;
  if (s === "sad") return 12;
  return 12; // idle / work
}

export function usePetFps(state: { value: PetState }) {
  const frame = ref(0);
  let timer: number | null = null;

  function tick() {
    frame.value = (frame.value + 1) % 1000;
  }

  function restart() {
    if (timer !== null) window.clearInterval(timer);
    const fps = getFps(state.value);
    // Respect visibility: when hidden, drop to 5fps
    const effectiveFps = document.visibilityState === "hidden" ? 5 : fps;
    timer = window.setInterval(tick, 1000 / effectiveFps);
  }

  watch(() => state.value, restart);

  function onVisibility() {
    restart();
  }

  onMounted(() => {
    restart();
    document.addEventListener("visibilitychange", onVisibility);
  });

  onUnmounted(() => {
    if (timer !== null) window.clearInterval(timer);
    document.removeEventListener("visibilitychange", onVisibility);
  });

  return { frame };
}
