<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from "vue";

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

function close() {
  emit("update:modelValue", false);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && props.modelValue) {
    close();
  }
}

watch(
  () => props.modelValue,
  (visible) => {
    document.body.classList.toggle("dialog-open", visible);
    if (visible) {
      window.addEventListener("keydown", onKeydown);
    } else {
      window.removeEventListener("keydown", onKeydown);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  document.body.classList.remove("dialog-open");
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-fade">
      <div v-if="modelValue" class="ui-dialog" @click.self="closeOnOverlay ? close() : undefined">
        <div class="ui-dialog__panel" :style="dialogStyle" role="dialog" aria-modal="true">
          <div class="ui-dialog__header">
            <div class="ui-dialog__title">
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
  background: rgba(31, 24, 20, 0.38);
  padding: 0.75rem;
  backdrop-filter: blur(0.375rem);
}

.ui-dialog__panel {
  max-height: calc(100vh - 1.5rem);
  overflow: auto;
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  box-shadow: 0 1.5rem 4rem rgba(48, 34, 24, 0.22);
}

.ui-dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1.125rem 1.25rem;
  border-bottom: 1px solid var(--color-border);
}

.ui-dialog__title :deep(h2) {
  margin: 0;
  font-size: 1.2rem;
  color: var(--color-heading);
}

.ui-dialog__body {
  padding: 1.25rem;
}

.ui-dialog__close {
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  transition:
    background-color 0.25s ease,
    color 0.25s ease,
    border-color 0.25s ease;
}

.ui-dialog__close:hover {
  background: var(--color-panel-muted);
  color: var(--color-heading);
  border-color: var(--color-border);
}

.ui-dialog__close:focus-visible {
  outline: none;
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.dialog-fade-enter-active,
.dialog-fade-leave-active {
  transition: opacity 0.22s ease;
}

.dialog-fade-enter-active .ui-dialog__panel,
.dialog-fade-leave-active .ui-dialog__panel {
  transition:
    transform 0.22s ease,
    opacity 0.22s ease;
}

.dialog-fade-enter-from,
.dialog-fade-leave-to {
  opacity: 0;
}

.dialog-fade-enter-from .ui-dialog__panel,
.dialog-fade-leave-to .ui-dialog__panel {
  opacity: 0;
  transform: translateY(0.75rem) scale(0.985);
}
</style>
