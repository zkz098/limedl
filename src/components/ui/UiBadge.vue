<script setup lang="ts">
withDefaults(
  defineProps<{
    size?: "sm" | "md";
    tone?: "neutral" | "info" | "success" | "warning" | "danger";
  }>(),
  {
    size: "md",
    tone: "neutral",
  },
);
</script>

<template>
  <span class="ui-badge" :class="[`ui-badge--${tone}`, `ui-badge--${size}`]">
    <slot />
  </span>
</template>

<style scoped>
.ui-badge {
  align-items: center;
  display: inline-flex;
  border-radius: var(--radius-sm);
  border: 1px solid var(--color-border);
  font-weight: 500;
  letter-spacing: 0;
  text-transform: none;
}

.ui-badge--md {
  min-height: 1.5rem;
  padding: 0 0.5rem;
  font-size: var(--font-size-label);
}

.ui-badge--sm {
  min-height: 1.25rem;
  padding: 0 0.375rem;
  font-size: 0.7rem;
}

.ui-badge--neutral {
  background: var(--color-panel-muted);
  color: var(--color-text-muted);
}

.ui-badge--info {
  background: var(--color-info-bg);
  color: var(--color-info-text);
  border-color: var(--color-info-border);
}

.ui-badge--success {
  background: var(--color-success-bg);
  color: var(--color-success-text);
  border-color: var(--color-success-border);
}

.ui-badge--warning {
  background: var(--color-warning-bg);
  color: var(--color-warning-text);
  border-color: var(--color-warning-border);
}

.ui-badge--danger {
  background: var(--color-danger-bg);
  color: var(--color-danger-text);
  border-color: var(--color-danger-border);
}
</style>
