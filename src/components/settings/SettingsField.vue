<script setup lang="ts">
import { provide, useId } from "vue";
import InfoTooltip from "../ui/InfoTooltip.vue";
import { FIELD_ASSOCIATION } from "../ui/field-association";

const props = defineProps<{
  label?: string;
  hint?: string;
  infoTooltip?: string;
  wide?: boolean;
  noAssociation?: boolean;
}>();

// Stable per-instance id linking the visible label to its slotted control.
const fieldId = useId();
const labelId = `${fieldId}-label`;
if (!props.noAssociation) {
  provide(FIELD_ASSOCIATION, { id: fieldId, labelId });
}
</script>

<template>
  <div class="settings-field" :class="{ 'settings-field--wide': wide }">
    <label v-if="label" :id="labelId" :for="fieldId" class="settings-field__label">
      {{ label }}
      <InfoTooltip v-if="infoTooltip" :text="infoTooltip" />
    </label>
    <slot :id="fieldId" :label-id="labelId" />
    <p v-if="hint" class="settings-field__hint">{{ hint }}</p>
  </div>
</template>
