<script setup lang="ts">
defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  close: [];
}>();

function handleClose() {
  emit("update:modelValue", false);
  emit("close");
}

function handleOverlayClick(event: MouseEvent) {
  if ((event.target as HTMLElement).classList.contains("fullscreen-overlay")) {
    handleClose();
  }
}
</script>

<template>
  <Transition name="overlay-fade">
    <div v-if="modelValue" class="fullscreen-overlay" @click="handleOverlayClick">
      <div class="modal-panel">
        <button type="button" class="overlay-close" @click="handleClose">
          <i class="i-ri-close-line" />
        </button>
        <div class="modal-panel__body">
          <slot />
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.fullscreen-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  overflow: auto;
  background: var(--surface-overlay-bg);
  -webkit-backdrop-filter: blur(8px);
  backdrop-filter: blur(8px);
}

.modal-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 64rem;
  height: calc(100vh - 2 * var(--space-6));
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  box-shadow:
    0 8px 32px oklch(0 0 0 / 0.12),
    0 2px 8px oklch(0 0 0 / 0.08);
  overflow: hidden;
}

.modal-panel__body {
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
  padding: var(--space-4) var(--space-4) 0;
}

.overlay-close {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  z-index: 10;
  width: 2.25rem;
  height: 2.25rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-panel);
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 1.125rem;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.overlay-close:hover {
  background: var(--color-surface-muted);
  color: var(--color-text-main);
}

.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 0.2s ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
}

.overlay-fade-enter-active .modal-panel,
.overlay-fade-leave-active .modal-panel {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}

.overlay-fade-enter-from .modal-panel,
.overlay-fade-leave-to .modal-panel {
  transform: scale(0.97);
  opacity: 0;
}

@media (max-width: 680px) {
  .fullscreen-overlay {
    padding: var(--space-4);
  }

  .modal-panel {
    max-height: calc(100vh - 2 * var(--space-4));
  }

  .modal-panel__body {
    padding: var(--space-3) var(--space-3) 0;
  }
}
</style>
