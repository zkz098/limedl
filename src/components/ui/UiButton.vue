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
  border-radius: var(--radius-md);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-weight: 600;
  gap: 0.5rem;
  justify-content: center;
  letter-spacing: -0.01em;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    background-color 0.2s ease,
    color 0.2s ease;
}

.ui-button:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.ui-button--md {
  min-height: 2.25rem;
  padding: 0 0.875rem;
}

.ui-button--sm {
  min-height: 1.875rem;
  padding: 0 0.625rem;
  font-size: var(--font-size-small);
}

.ui-button--block {
  width: 100%;
}

.ui-button--primary {
  background: var(--color-accent);
  color: var(--color-accent-contrast);
}

.ui-button--primary:hover:not(:disabled) {
  background: var(--color-accent-strong);
}

.ui-button--secondary {
  background: var(--color-panel);
  border-color: var(--color-border);
  color: var(--color-text-main);
}

.ui-button--secondary:hover:not(:disabled) {
  border-color: var(--color-border-strong);
  background: var(--color-surface-muted);
}

.ui-button--ghost {
  background: transparent;
  border-color: transparent;
  color: var(--color-text-muted);
}

.ui-button--ghost:hover:not(:disabled) {
  color: var(--color-text-main);
  background: var(--color-surface-muted);
}

.ui-button--danger {
  background: var(--color-danger-bg);
  border-color: var(--color-danger-border);
  color: var(--color-danger-text);
}

.ui-button--danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--color-danger-bg) 92%, var(--color-text-main));
}

.ui-button__icon {
  font-size: 1rem;
}

.ui-button__label {
  min-width: 0;
}

.ui-button__spinner {
  width: 0.9rem;
  height: 0.9rem;
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
