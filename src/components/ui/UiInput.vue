<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue: string;
    type?: string;
    placeholder?: string;
    readonly?: boolean;
    disabled?: boolean;
    inputmode?: "none" | "text" | "decimal" | "numeric" | "tel" | "search" | "email" | "url";
  }>(),
  {
    type: "text",
    placeholder: "",
    readonly: false,
    disabled: false,
    inputmode: undefined,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();
</script>

<template>
  <input
    :value="modelValue"
    class="ui-input"
    :type="type"
    :placeholder="placeholder"
    :readonly="readonly"
    :disabled="disabled"
    :inputmode="inputmode"
    @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
  />
</template>

<style scoped>
.ui-input {
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

.ui-input::placeholder {
  color: var(--color-text-soft);
}

.ui-input:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-input[readonly] {
  color: var(--color-text-muted);
  background: var(--color-panel-muted);
}

.ui-input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
