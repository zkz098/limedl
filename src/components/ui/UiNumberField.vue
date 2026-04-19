<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: number | null;
    min?: number;
    max?: number;
    step?: number;
    placeholder?: string;
    disabled?: boolean;
  }>(),
  {
    min: undefined,
    max: undefined,
    step: 1,
    placeholder: "",
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: number | null];
}>();

function onInput(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  emit("update:modelValue", value === "" ? null : Number(value));
}
</script>

<template>
  <input
    :value="modelValue ?? ''"
    class="ui-input"
    type="number"
    inputmode="numeric"
    :min="min"
    :max="max"
    :step="step"
    :placeholder="placeholder"
    :disabled="disabled"
    @input="onInput"
  />
</template>

<style scoped>
.ui-input {
  width: 100%;
  min-height: 2.875rem;
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  padding: 0 0.9375rem;
  transition:
    border-color 0.25s ease,
    box-shadow 0.25s ease,
    background-color 0.25s ease;
}

.ui-input:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 0.1875rem var(--color-focus-ring);
}

.ui-input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
