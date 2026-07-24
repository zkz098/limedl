import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent, ref, type Ref } from "vue";
import { useFloatingClose } from "../../composables/useFloatingClose";

// ── Helper ─────────────────────────────────────────────────────────────────────

function mountComposable(
  panelRef: Ref<HTMLElement | null>,
  isOpen: Ref<boolean>,
  onClose: () => void,
) {
  return mount(
    defineComponent({
      setup() {
        useFloatingClose(panelRef, isOpen, onClose);
        return () => {};
      },
    }),
  );
}

// ── Tests ───────────────────────────────────────────────────────────────────────

describe("useFloatingClose", () => {
  let panelRef: Ref<HTMLElement | null>;
  let isOpen: Ref<boolean>;
  let onClose: () => void;
  let panelEl: HTMLElement;
  let outsideEl: HTMLElement;
  let insideEl: HTMLElement;

  beforeEach(() => {
    panelRef = ref<HTMLElement | null>(null);
    isOpen = ref(false);
    onClose = vi.fn();

    panelEl = document.createElement("div");
    outsideEl = document.createElement("div");
    insideEl = document.createElement("div");
    panelEl.appendChild(insideEl);
    document.body.appendChild(panelEl);
    document.body.appendChild(outsideEl);
  });

  afterEach(() => {
    // Remove test elements to keep DOM clean
    if (panelEl.parentNode) panelEl.parentNode.removeChild(panelEl);
    if (outsideEl.parentNode) outsideEl.parentNode.removeChild(outsideEl);
  });

  // ── Click outside panel ────────────────────────────────────────────────

  describe("pointerdown outside panel", () => {
    it("calls onClose when isOpen is true", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("does NOT call onClose when isOpen is false", () => {
      isOpen.value = false;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

      expect(onClose).not.toHaveBeenCalled();
    });

    it("does NOT call onClose when clicking inside the panel", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      insideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

      expect(onClose).not.toHaveBeenCalled();
    });

    it("does NOT call onClose when target is not an HTMLElement", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      // Dispatching on document sets target=Document (not HTMLElement)
      document.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

      expect(onClose).not.toHaveBeenCalled();
    });
  });

  // ── Escape key ──────────────────────────────────────────────────────────

  describe("Escape keydown", () => {
    it("calls onClose when isOpen is true", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("does NOT call onClose when isOpen is false", () => {
      isOpen.value = false;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

      expect(onClose).not.toHaveBeenCalled();
    });

    it("does NOT call onClose for non-Escape keys", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      mountComposable(panelRef, isOpen, onClose);

      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));

      expect(onClose).not.toHaveBeenCalled();
    });
  });

  // ── Lifecycle ───────────────────────────────────────────────────────────

  describe("lifecycle", () => {
    it("onUnmounted removes event listeners (no calls after unmount)", () => {
      isOpen.value = true;
      panelRef.value = panelEl;
      const wrapper = mountComposable(panelRef, isOpen, onClose);

      // Confirm listener works before unmount
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).toHaveBeenCalledTimes(1);

      wrapper.unmount();

      // After unmount, listeners should be removed
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).toHaveBeenCalledTimes(1); // still 1
    });

    it("registers both pointerdown and keydown listeners on mount", () => {
      const addSpy = vi.spyOn(document, "addEventListener");
      isOpen.value = true;
      panelRef.value = panelEl;

      mountComposable(panelRef, isOpen, onClose);

      expect(addSpy).toHaveBeenCalledWith("pointerdown", expect.any(Function));
      expect(addSpy).toHaveBeenCalledWith("keydown", expect.any(Function));
      addSpy.mockRestore();
    });
  });

  // ── Edge cases ──────────────────────────────────────────────────────────

  describe("edge cases", () => {
    it("does not call onClose when panelRef is null", () => {
      panelRef.value = null; // explicitly null
      isOpen.value = true;
      mountComposable(panelRef, isOpen, onClose);

      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));

      // When panelRef.value is null, the condition
      // `panelRef.value && !panelRef.value.contains(target)` short-circuits
      expect(onClose).not.toHaveBeenCalled();
    });

    it("handles rapid open/close toggles without leaking listeners", () => {
      isOpen.value = true;
      panelRef.value = panelEl;

      const wrapper = mountComposable(panelRef, isOpen, onClose);

      // Toggle open/close state repeatedly
      isOpen.value = false;
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).not.toHaveBeenCalled();

      isOpen.value = true;
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).toHaveBeenCalledTimes(1);

      // Toggle again
      isOpen.value = false;
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).toHaveBeenCalledTimes(1); // still 1

      wrapper.unmount();
    });

    it("works when panelRef is set after mount", () => {
      panelRef.value = null; // panel ref set after mount
      isOpen.value = true;
      mountComposable(panelRef, isOpen, onClose);

      // Panel ref not set yet — onClose should NOT be called (null guard)
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      expect(onClose).not.toHaveBeenCalled();

      // Now set the panel ref
      panelRef.value = panelEl;
      outsideEl.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      // Still not called because panelRef.value contains outsideEl... wait no.
      // outsideEl is NOT inside panelEl, so onClose is called
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });

  // ── Return value ────────────────────────────────────────────────────────

  describe("return value", () => {
    it("returns undefined (void)", () => {
      isOpen.value = true;
      panelRef.value = panelEl;

      // The composable does not return anything, so calling it via
      // defineComponent is sufficient. We just verify it exists.
      let result: ReturnType<typeof useFloatingClose> | undefined;
      mount(
        defineComponent({
          setup() {
            result = useFloatingClose(panelRef, isOpen, onClose);
            return () => {};
          },
        }),
      );

      expect(result).toBeUndefined();
    });
  });
});
