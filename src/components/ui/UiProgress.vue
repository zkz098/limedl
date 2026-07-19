<script setup lang="ts">
defineProps<{
  value: number;
  indeterminate?: boolean;
  showLabel?: boolean;
  label?: string;
}>();
</script>

<template>
  <div class="ui-progress">
    <div class="ui-progress__track">
      <div
        class="ui-progress__value"
        data-testid="task-progress-bar"
        :class="{ 'ui-progress__value--indeterminate': indeterminate }"
        :style="indeterminate ? undefined : { width: `${Math.max(0, Math.min(value, 100))}%` }"
      />
    </div>
    <span v-if="showLabel" class="ui-progress__label">{{ label ?? `${value.toFixed(1)}%` }}</span>
  </div>
</template>

<style scoped>
.ui-progress {
  display: grid;
  gap: 0.35rem;
}

.ui-progress__track {
  height: 0.4rem;
  background: var(--color-progress-track);
  border-radius: var(--radius-pill);
  overflow: hidden;
}

.ui-progress__value {
  height: 100%;
  border-radius: inherit;
  background: var(--color-accent);
  transition: width 0.3s ease;
}

.ui-progress__value--indeterminate {
  width: 100%;
  background: linear-gradient(
    90deg,
    var(--color-accent-strong) 0%,
    var(--color-accent) 50%,
    var(--color-accent-strong) 100%
  );
  background-size: 200% 100%;
  animation: ui-progress-indeterminate 1.6s ease infinite;
}

@keyframes ui-progress-indeterminate {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ui-progress__value--indeterminate {
    animation: none;
    background: var(--color-progress-track);
    position: relative;
  }

  .ui-progress__value--indeterminate::after {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    bottom: 0;
    width: 35%;
    border-radius: inherit;
    background: var(--color-accent);
  }
}

.ui-progress__label {
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}
</style>
