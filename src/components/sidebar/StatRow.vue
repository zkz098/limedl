<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string;
    value: string | number;
    mono?: boolean;
  }>(),
  {
    mono: false,
  },
);
</script>

<template>
  <div class="stat-row">
    <span class="stat-row__label">{{ label }}</span>
    <span class="stat-row__value" :class="{ 'stat-row__value--mono': mono }">{{ value }}</span>
  </div>
</template>

<style scoped>
.stat-row {
  display: flex;
  justify-content: space-between;
  padding: 0.15rem 0;
  font-size: 0.8rem;
}

.stat-row__label {
  color: var(--color-text-muted);
}

.stat-row__value {
  font-weight: 600;
}

.stat-row__value--mono {
  font-family: var(--font-body);
  font-size: 0.75rem;
}
</style>
