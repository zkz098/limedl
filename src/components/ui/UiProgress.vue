<script setup lang="ts">
defineProps<{
  value: number;
  showLabel?: boolean;
  label?: string;
}>();
</script>

<template>
  <div class="ui-progress">
    <div class="ui-progress__track">
      <div class="ui-progress__value" :style="{ width: `${Math.max(0, Math.min(value, 100))}%` }" />
    </div>
    <span v-if="showLabel" class="ui-progress__label">{{ label ?? `${value.toFixed(1)}%` }}</span>
  </div>
</template>

<style scoped>
.ui-progress {
  display: grid;
  gap: 0.5rem;
}

.ui-progress__track {
  height: 0.6rem;
  background: var(--color-progress-track);
  border-radius: 999px;
  overflow: hidden;
}

.ui-progress__value {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--color-accent-strong), var(--color-accent));
  transition: width 0.35s ease;
}

.ui-progress__label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}
</style>
