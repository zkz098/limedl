<script lang="ts">
// Module-level singleton: only one InfoTooltip may be open at a time.
// Declared outside <script setup> so it's shared across all component instances.
import { ref } from "vue";
const activeTrigger = ref<HTMLElement | null>(null);
</script>

<script setup lang="ts">
import { arrow, autoUpdate, computePosition, flip, offset, shift } from "@floating-ui/dom";
import { computed, nextTick, onUnmounted, ref, useId, watch } from "vue";
import { useFloatingClose } from "../../composables/useFloatingClose";
import { t } from "../../i18n";

const props = defineProps<{
  text: string;
}>();

const tooltipId = useId();

const triggerRef = ref<HTMLButtonElement | null>(null);
const popupRef = ref<HTMLDivElement | null>(null);
const arrowRef = ref<HTMLDivElement | null>(null);

// Module-level singleton: only one InfoTooltip may be open at a time.
// activeTrigger is declared above at module scope, outside setup.

const isOpen = computed(() => {
  const trigger = triggerRef.value;
  return trigger != null && activeTrigger.value === trigger;
});
const isPinned = ref(false);

let hoverShowTimer: number | null = null;
let hoverHideTimer: number | null = null;
const isHovering = ref(false);

const ARROW_SIZE = 8;
let cleanupAutoUpdate: (() => void) | null = null;

function isTouchDevice() {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(pointer: coarse)").matches;
}

function clearHoverShowTimer() {
  if (hoverShowTimer) {
    clearTimeout(hoverShowTimer);
    hoverShowTimer = null;
  }
}

function clearHoverHideTimer() {
  if (hoverHideTimer) {
    clearTimeout(hoverHideTimer);
    hoverHideTimer = null;
  }
}

function open(source: "hover" | "click") {
  activeTrigger.value = triggerRef.value;
  if (source === "click") {
    isPinned.value = true;
  }
}

function close() {
  if (activeTrigger.value === triggerRef.value) {
    activeTrigger.value = null;
  }
  isPinned.value = false;
}

function onMouseEnter() {
  if (isTouchDevice()) return;
  isHovering.value = true;
  clearHoverHideTimer();
  clearHoverShowTimer();
  hoverShowTimer = window.setTimeout(() => {
    if (isHovering.value) {
      open("hover");
    }
  }, 300);
}

function onMouseLeave() {
  isHovering.value = false;
  clearHoverShowTimer();
  clearHoverHideTimer();
  hoverHideTimer = window.setTimeout(() => {
    if (!isHovering.value && !isPinned.value) {
      close();
    }
  }, 150);
}

function onPopupEnter() {
  isHovering.value = true;
  clearHoverHideTimer();
}

function onPopupLeave() {
  isHovering.value = false;
  clearHoverHideTimer();
  hoverHideTimer = window.setTimeout(() => {
    if (!isHovering.value && !isPinned.value) {
      close();
    }
  }, 150);
}

function onPointerDown(e: PointerEvent) {
  e.stopPropagation();
}

function onClick() {
  if (isOpen.value && isPinned.value) {
    close();
  } else {
    open("click");
  }
}

async function updatePosition() {
  if (!triggerRef.value || !popupRef.value || !arrowRef.value) return;
  const { x, y, middlewareData, placement } = await computePosition(
    triggerRef.value,
    popupRef.value,
    {
      strategy: "fixed",
      placement: "top",
      middleware: [
        offset(ARROW_SIZE),
        flip(),
        shift({ padding: 8 }),
        arrow({ element: arrowRef.value, padding: 4 }),
      ],
    },
  );

  popupRef.value.style.transform = `translate3d(${Math.round(x)}px, ${Math.round(y)}px, 0)`;

  const { x: arrowX, y: arrowY } = middlewareData.arrow ?? {};
  const side = placement.split("-")[0];
  const style = arrowRef.value.style;
  style.left = style.top = style.right = style.bottom = "";

  if (side === "top") {
    style.left = arrowX != null ? `${arrowX}px` : "50%";
    style.top = "100%";
  } else if (side === "bottom") {
    style.left = arrowX != null ? `${arrowX}px` : "50%";
    style.top = "0";
  } else if (side === "left") {
    style.left = "100%";
    style.top = arrowY != null ? `${arrowY}px` : "50%";
  } else if (side === "right") {
    style.left = "0";
    style.top = arrowY != null ? `${arrowY}px` : "50%";
  }
}

watch(isOpen, (opened) => {
  if (opened) {
    nextTick(() => {
      if (!isOpen.value) return;
      updatePosition();
      if (triggerRef.value && popupRef.value) {
        cleanupAutoUpdate = autoUpdate(triggerRef.value, popupRef.value, updatePosition);
      }
    });
  } else if (cleanupAutoUpdate) {
    cleanupAutoUpdate();
    cleanupAutoUpdate = null;
  }
});

useFloatingClose(popupRef, isOpen, close);

onUnmounted(() => {
  if (activeTrigger.value === triggerRef.value) {
    activeTrigger.value = null;
  }
  clearHoverShowTimer();
  clearHoverHideTimer();
  if (cleanupAutoUpdate) {
    cleanupAutoUpdate();
    cleanupAutoUpdate = null;
  }
});
</script>

<template>
  <span class="info-tooltip">
    <button
      ref="triggerRef"
      type="button"
      class="info-tooltip__icon"
      :class="{ 'is-open': isOpen }"
      :aria-label="t('common.information')"
      :aria-describedby="isOpen ? tooltipId : undefined"
      @mouseenter="onMouseEnter"
      @mouseleave="onMouseLeave"
      @pointerdown="onPointerDown"
      @click.stop="onClick"
    >
      <span class="i-ri-information-line" aria-hidden="true" />
    </button>

    <Teleport to="body">
      <Transition name="tooltip-fade">
        <div
          v-show="isOpen"
          :id="tooltipId"
          ref="popupRef"
          class="info-tooltip__popup"
          role="tooltip"
          @mouseenter="onPopupEnter"
          @mouseleave="onPopupLeave"
        >
          <div class="info-tooltip__content">{{ props.text }}</div>
          <div ref="arrowRef" class="info-tooltip__arrow" />
        </div>
      </Transition>
    </Teleport>
  </span>
</template>

<style scoped>
.info-tooltip {
  display: inline-flex;
  vertical-align: middle;
  line-height: 1;
}

.info-tooltip__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1rem;
  height: 1rem;
  padding: 0;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-muted);
  font-size: 1rem;
  cursor: pointer;
  transition: color var(--duration-fast) ease;
}

.info-tooltip__icon:hover,
.info-tooltip__icon:focus-visible,
.info-tooltip__icon.is-open {
  color: var(--color-text-main);
}

.info-tooltip__icon:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.info-tooltip__popup {
  position: fixed;
  top: 0;
  left: 0;
  z-index: 150;
  max-width: min(280px, calc(100vw - 2rem));
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--color-tooltip-bg);
  color: var(--color-tooltip-text);
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  box-shadow: var(--shadow-card-hover);
  pointer-events: auto;
}

.info-tooltip__content {
  white-space: pre-line;
}

.info-tooltip__arrow {
  position: absolute;
  width: 8px;
  height: 8px;
  background: inherit;
  transform: translate(-50%, -50%) rotate(45deg);
}

.tooltip-fade-enter-active,
.tooltip-fade-leave-active {
  transition: opacity var(--duration-fast) ease;
}

.tooltip-fade-enter-from,
.tooltip-fade-leave-to {
  opacity: 0;
}
</style>
