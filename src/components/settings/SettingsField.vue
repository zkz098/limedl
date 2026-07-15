<script setup lang="ts">
defineProps<{
  label?: string;
  hint?: string;
  wide?: boolean;
}>();
</script>

<template>
  <div class="settings-field" :class="{ 'settings-field--wide': wide }">
    <span v-if="label" class="settings-field__label">{{ label }}</span>
    <slot />
    <p v-if="hint" class="settings-field__hint">{{ hint }}</p>
  </div>
</template>
