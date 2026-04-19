<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    type?: "button" | "submit" | "reset";
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "sm" | "md";
    loading?: boolean;
    disabled?: boolean;
    icon?: string;
    iconRight?: string;
    block?: boolean;
  }>(),
  {
    type: "button",
    variant: "primary",
    size: "md",
    loading: false,
    disabled: false,
    icon: "",
    iconRight: "",
    block: false,
  },
);

defineEmits<{
  click: [event: MouseEvent];
}>();

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<template>
  <button
    :type="type"
    class="ui-button"
    :class="[
      `ui-button--${variant}`,
      `ui-button--${size}`,
      {
        'ui-button--block': block,
        'is-loading': loading,
      },
    ]"
    :disabled="isDisabled"
    @click="$emit('click', $event)"
  >
    <span v-if="loading" class="ui-button__spinner" aria-hidden="true" />
    <span v-else-if="icon" class="ui-button__icon" :class="icon" aria-hidden="true" />
    <span v-if="$slots.default" class="ui-button__label">
      <slot />
    </span>
    <span
      v-if="!loading && iconRight"
      class="ui-button__icon"
      :class="iconRight"
      aria-hidden="true"
    />
  </button>
</template>

<style scoped>
.ui-button {
  align-items: center;
  appearance: none;
  border: 1px solid transparent;
  border-radius: var(--radius-pill);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-weight: 600;
  gap: 0.625rem;
  justify-content: center;
  letter-spacing: 0.01em;
  transition:
    transform 0.25s ease,
    border-color 0.25s ease,
    box-shadow 0.25s ease,
    background-color 0.25s ease,
    color 0.25s ease;
}

.ui-button:hover:not(:disabled) {
  transform: translateY(-0.0625rem);
}

.ui-button:focus-visible {
  outline: none;
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.ui-button:disabled {
  cursor: not-allowed;
  opacity: 0.56;
  transform: none;
}

.ui-button--md {
  min-height: 2.75rem;
  padding: 0 1rem;
}

.ui-button--sm {
  min-height: 2.125rem;
  padding: 0 0.875rem;
}

.ui-button--block {
  width: 100%;
}

.ui-button--primary {
  background: linear-gradient(135deg, var(--color-accent-strong), var(--color-accent));
  box-shadow: var(--shadow-accent);
  color: var(--color-accent-contrast);
}

.ui-button--primary:hover:not(:disabled) {
  box-shadow: 0 0.75rem 1.5rem rgba(180, 108, 92, 0.22);
}

.ui-button--secondary {
  background: var(--color-panel-muted);
  border-color: var(--color-border);
  color: var(--color-text-main);
}

.ui-button--secondary:hover:not(:disabled) {
  border-color: var(--color-accent-soft-border);
  background: var(--color-surface-hover);
}

.ui-button--ghost {
  background: transparent;
  border-color: transparent;
  color: var(--color-text-muted);
}

.ui-button--ghost:hover:not(:disabled) {
  color: var(--color-accent-strong);
  background: var(--color-panel-muted);
}

.ui-button--danger {
  background: var(--color-danger-bg);
  border-color: var(--color-danger-border);
  color: var(--color-danger-text);
}

.ui-button--danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-danger-bg) 70%, white);
}

.ui-button__icon {
  font-size: 1rem;
}

.ui-button__label {
  min-width: 0;
}

.ui-button__spinner {
  width: 0.95rem;
  height: 0.95rem;
  border: 0.125rem solid currentColor;
  border-right-color: transparent;
  border-radius: 999px;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
