<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: number | null;
    min?: number;
    max?: number;
    step?: number;
    placeholder?: string;
    disabled?: boolean;
    unit?: string;
    unitPosition?: "suffix" | "prefix";
  }>(),
  {
    min: undefined,
    max: undefined,
    step: 1,
    placeholder: "",
    disabled: false,
    unit: undefined,
    unitPosition: "suffix",
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
  <div
    class="ui-unit-input"
    :class="{ 'ui-unit-input--disabled': disabled }"
  >
    <span
      v-if="unit && unitPosition === 'prefix'"
      class="ui-unit-input__affix ui-unit-input__prefix"
    >{{ unit }}</span>
    <input
      :value="modelValue ?? ''"
      class="ui-unit-input__field"
      type="number"
      inputmode="numeric"
      :min="min"
      :max="max"
      :step="step"
      :placeholder="placeholder"
      :disabled="disabled"
      @input="onInput"
    />
    <span
      v-if="unit && unitPosition === 'suffix'"
      class="ui-unit-input__affix ui-unit-input__suffix"
    >{{ unit }}</span>
  </div>
</template>

<style scoped>
.ui-unit-input {
  display: flex;
  align-items: center;
  min-height: 2.25rem;
  padding: 0 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.ui-unit-input:focus-within {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-unit-input--disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ui-unit-input__field {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: none;
  outline: none;
  background: transparent;
  font: inherit;
  color: var(--color-text-main);
}

.ui-unit-input__field:disabled {
  cursor: not-allowed;
}

.ui-unit-input__field::-webkit-outer-spin-button,
.ui-unit-input__field::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.ui-unit-input__field[type="number"] {
  -moz-appearance: textfield;
}

.ui-unit-input__affix {
  flex: 0 0 auto;
  margin-left: 0.5rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: 0.85rem;
  white-space: nowrap;
  user-select: none;
}

.ui-unit-input__prefix {
  margin-left: 0;
  margin-right: 0.5rem;
}
</style>
