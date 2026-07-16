<script setup lang="ts">
import InfoTooltip from "../ui/InfoTooltip.vue";

defineProps<{
  label?: string;
  hint?: string;
  infoTooltip?: string;
  wide?: boolean;
}>();
</script>

<template>
  <div class="settings-field" :class="{ 'settings-field--wide': wide }">
    <span v-if="label" class="settings-field__label">
      {{ label }}
      <InfoTooltip v-if="infoTooltip" :text="infoTooltip" />
    </span>
    <slot />
    <p v-if="hint" class="settings-field__hint">{{ hint }}</p>
  </div>
</template>
