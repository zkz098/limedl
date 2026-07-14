<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    label?: string;
    disabled?: boolean;
  }>(),
  {
    label: "",
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

function onInputChange(event: Event): void {
  const target = event.target as HTMLInputElement;
  emit("update:modelValue", target.checked);
}
</script>

<template>
  <label
    class="ui-switch"
    :class="{ 'ui-switch--disabled': disabled }"
    :aria-disabled="disabled"
  >
    <span v-if="label || $slots.default" class="ui-switch__label">
      <slot>{{ label }}</slot>
    </span>
    <input
      type="checkbox"
      class="ui-switch__input"
      :checked="modelValue"
      :disabled="disabled"
      @change="onInputChange"
    />
    <span class="ui-switch__track" aria-hidden="true">
      <span class="ui-switch__thumb" />
    </span>
  </label>
</template>

<style scoped>
.ui-switch {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  cursor: pointer;
  min-height: 1.5rem;
}

.ui-switch--disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.ui-switch__label {
  color: var(--color-heading);
  font-size: var(--font-size-small);
  font-weight: 500;
  line-height: 1.4;
}

.ui-switch__input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}

.ui-switch__track {
  position: relative;
  flex: 0 0 auto;
  width: 2.25rem;
  height: 1.25rem;
  display: inline-block;
  border-radius: var(--radius-pill);
  background: var(--color-border-strong);
  transition: background-color 0.2s ease;
}

.ui-switch__input:checked + .ui-switch__track {
  background: var(--color-accent-strong);
}

.ui-switch__input:focus-visible + .ui-switch__track {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-switch__thumb {
  position: absolute;
  top: 0.125rem;
  left: 0.125rem;
  width: 1rem;
  height: 1rem;
  border-radius: var(--radius-pill);
  background: var(--color-panel);
  box-shadow: var(--shadow-soft);
  transition: transform 0.2s ease;
}

.ui-switch__input:checked + .ui-switch__track .ui-switch__thumb {
  transform: translateX(1rem);
}
</style>
