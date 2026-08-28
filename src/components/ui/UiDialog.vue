<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from "vue";

import { useI18n } from "../../i18n";

const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    title?: string;
    width?: string;
    closeOnOverlay?: boolean;
  }>(),
  {
    title: "",
    width: "min(42rem, calc(100vw - 1.5rem))",
    closeOnOverlay: true,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

const { t } = useI18n();
const dialogStyle = computed(() => ({ width: props.width }));
const panelRef = ref<HTMLElement | null>(null);
const titleId = useId();
let previouslyFocused: HTMLElement | null = null;

const FOCUSABLE_SELECTOR = [
  "[autofocus]",
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

function close() {
  emit("update:modelValue", false);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && props.modelValue) {
    close();
  }
}

// aria-modal requires focus containment: keep Tab/Shift+Tab within the dialog.
function onFocusTrapKeydown(event: KeyboardEvent) {
  if (event.key !== "Tab") return;
  const panel = panelRef.value;
  if (!panel) return;
  const focusables = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
  if (focusables.length === 0) return;
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

watch(
  () => props.modelValue,
  async (visible) => {
    document.body.classList.toggle("dialog-open", visible);
    if (visible) {
      previouslyFocused = document.activeElement as HTMLElement | null;
      window.addEventListener("keydown", onKeydown);
      window.addEventListener("keydown", onFocusTrapKeydown);
      await nextTick();
      const initialFocus = panelRef.value?.querySelector<HTMLElement>("[autofocus]")
        ?? panelRef.value?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR);
      initialFocus?.focus();
    } else {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("keydown", onFocusTrapKeydown);
      previouslyFocused?.focus?.();
      previouslyFocused = null;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.body.classList.remove("dialog-open");
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("keydown", onFocusTrapKeydown);
  previouslyFocused?.focus?.();
});
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-fade">
      <div v-if="modelValue" class="ui-dialog" @click.self="closeOnOverlay ? close() : undefined">
        <div
          ref="panelRef"
          class="ui-dialog__panel"
          :style="dialogStyle"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="titleId"
        >
          <div class="ui-dialog__header">
            <div :id="titleId" class="ui-dialog__title">
              <slot name="title">
                <h2>{{ title }}</h2>
              </slot>
            </div>
            <button
              type="button"
              class="ui-dialog__close"
              :aria-label="t('common.close')"
              @click="close"
            >
              <span class="i-ri-close-line" aria-hidden="true" />
            </button>
          </div>
          <div class="ui-dialog__body">
            <slot />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.ui-dialog {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  background: var(--surface-overlay-bg);
  padding: 0.75rem;
}

.ui-dialog__panel {
  max-height: calc(100vh - 1.5rem);
  overflow: auto;
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card-hover);
}

.ui-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--color-border);
}

.ui-dialog__title :deep(h2) {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.ui-dialog__body {
  padding: 1.25rem;
}

.ui-dialog__close {
  width: 2rem;
  height: 2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  transition:
    background-color 0.2s ease,
    color 0.2s ease,
    border-color 0.2s ease;
}

.ui-dialog__close:hover {
  background: var(--color-surface-muted);
  color: var(--color-heading);
  border-color: var(--color-border);
}

.ui-dialog__close:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.dialog-fade-enter-active,
.dialog-fade-leave-active {
  transition: opacity 0.2s ease;
}

.dialog-fade-enter-active .ui-dialog__panel,
.dialog-fade-leave-active .ui-dialog__panel {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}

.dialog-fade-enter-from,
.dialog-fade-leave-to {
  opacity: 0;
}

.dialog-fade-enter-from .ui-dialog__panel,
.dialog-fade-leave-to .ui-dialog__panel {
  opacity: 0;
  transform: translateY(0.5rem);
}
</style>
