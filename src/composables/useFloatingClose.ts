import { onMounted, onUnmounted, type Ref } from "vue";

export function useFloatingClose(
  panelRef: Ref<HTMLElement | null>,
  isOpen: Ref<boolean>,
  onClose: () => void,
) {
  function handlePointerDown(e: PointerEvent) {
    if (!isOpen.value) return;
    const target = e.target;
    if (!(target instanceof HTMLElement)) return;
    if (panelRef.value && !panelRef.value.contains(target)) {
      onClose();
    }
  }

  function handleEscape(e: KeyboardEvent) {
    if (!isOpen.value) return;
    if (e.key === "Escape") {
      onClose();
    }
  }

  onMounted(() => {
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleEscape);
  });

  onUnmounted(() => {
    document.removeEventListener("pointerdown", handlePointerDown);
    document.removeEventListener("keydown", handleEscape);
  });
}
