<script setup lang="ts">
import { computed, inject } from "vue";
import { FIELD_ASSOCIATION } from "./field-association";

const props = withDefaults(
  defineProps<{
    modelValue: string | number | null;
    type?: "text" | "number" | "url";
    id?: string;
    ariaLabel?: string;
    ariaLabelledby?: string;
    placeholder?: string;
    disabled?: boolean;
    min?: number;
    max?: number;
    step?: number;
    unit?: string;
    unitPosition?: "prefix" | "suffix";
  }>(),
  {
    type: "text",
    placeholder: "",
    disabled: false,
    min: undefined,
    max: undefined,
    step: 1,
    unit: undefined,
    unitPosition: "suffix",
  },
);

// When rendered inside a SettingsField and no explicit id is given, inherit
// the field's generated id so the <label for> association works.
const fieldAssociation = inject(FIELD_ASSOCIATION, null);
const resolvedId = computed(() => props.id ?? fieldAssociation?.id ?? undefined);

const emit = defineEmits<{
  "update:modelValue": [value: string | number | null];
}>();

function onInput(event: Event) {
  const raw = (event.target as HTMLInputElement).value;
  if (props.type === "number") {
    emit("update:modelValue", raw === "" ? null : Number(raw));
  } else {
    emit("update:modelValue", raw);
  }
}
</script>

<template>
  <div
    v-if="unit"
    class="ui-textfield-wrapper"
    :class="{ 'ui-textfield-wrapper--disabled': disabled }"
  >
    <span v-if="unitPosition === 'prefix'" class="ui-textfield__affix ui-textfield__prefix">{{
      unit
    }}</span>
    <input
      :id="resolvedId"
      :aria-label="ariaLabel"
      :aria-labelledby="ariaLabelledby"
      :value="type === 'number' ? (modelValue ?? '') : modelValue"
      class="ui-textfield"
      :type="type"
      inputmode="numeric"
      :placeholder="placeholder"
      :disabled="disabled"
      :min="min"
      :max="max"
      :step="step"
      @input="onInput"
    />
    <span v-if="unitPosition === 'suffix'" class="ui-textfield__affix ui-textfield__suffix">{{
      unit
    }}</span>
  </div>
  <input
    v-else
    :id="resolvedId"
    :aria-label="ariaLabel"
    :aria-labelledby="ariaLabelledby"
    :value="type === 'number' ? (modelValue ?? '') : modelValue"
    class="ui-textfield"
    :type="type"
    :inputmode="type === 'number' ? 'numeric' : undefined"
    :placeholder="placeholder"
    :disabled="disabled"
    :min="min"
    :max="max"
    :step="step"
    @input="onInput"
  />
</template>

<style scoped>
.ui-textfield {
  width: 100%;
  min-height: 2.25rem;
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  padding: 0 0.75rem;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease,
    background-color 0.2s ease;
}

.ui-textfield::placeholder {
  color: var(--color-text-soft);
}

.ui-textfield:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-textfield:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ui-textfield[readonly] {
  color: var(--color-text-muted);
  background: var(--color-panel-muted);
  cursor: default;
}

/* Hide spin buttons for number type */
.ui-textfield[type="number"]::-webkit-outer-spin-button,
.ui-textfield[type="number"]::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.ui-textfield[type="number"] {
  -moz-appearance: textfield;
}

/* Wrapper for unit variant */
.ui-textfield-wrapper {
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

.ui-textfield-wrapper:focus-within {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-textfield-wrapper--disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ui-textfield-wrapper .ui-textfield {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--color-text-main);
  font: inherit;
  min-height: auto;
}

.ui-textfield-wrapper .ui-textfield:focus-visible {
  outline: none;
  box-shadow: none;
  border-color: transparent;
}

.ui-textfield-wrapper .ui-textfield:disabled {
  cursor: not-allowed;
}

.ui-textfield__affix {
  flex: 0 0 auto;
  margin-left: 0.5rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: 0.85rem;
  white-space: nowrap;
  user-select: none;
}

.ui-textfield__prefix {
  margin-left: 0;
  margin-right: 0.5rem;
}
</style>
