<script setup lang="ts" generic="T extends string | number | null">
const props = defineProps<{
  modelValue: T;
  options: { label: string; value: T }[];
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: T];
}>();

function onChange(event: Event) {
  const rawValue = (event.target as HTMLSelectElement).value;
  const nextValue = props.options.find((option) => String(option.value) === rawValue)?.value;

  if (nextValue !== undefined) {
    emit("update:modelValue", nextValue);
  }
}
</script>

<template>
  <div class="ui-select">
    <select
      class="ui-select__control"
      :value="modelValue ?? ''"
      :disabled="disabled"
      @change="onChange"
    >
      <option v-for="option in options" :key="String(option.value)" :value="option.value">
        {{ option.label }}
      </option>
    </select>
    <span class="i-ri-arrow-down-s-line ui-select__icon" aria-hidden="true" />
  </div>
</template>

<style scoped>
.ui-select {
  position: relative;
}

.ui-select__control {
  appearance: none;
  width: 100%;
  min-height: 2.25rem;
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  padding: 0 2.25rem 0 0.75rem;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    background-color 0.2s ease;
}

.ui-select__control:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-select__control:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ui-select__icon {
  position: absolute;
  right: 0.625rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-muted);
  pointer-events: none;
}
</style>
