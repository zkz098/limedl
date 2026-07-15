<script setup lang="ts">
withDefaults(
  defineProps<{
    icon?: string;
    title: string;
    description?: string;
  }>(),
  {
    icon: undefined,
    description: undefined,
  },
);
</script>

<template>
  <div class="ui-empty-state">
    <span v-if="icon" class="ui-empty-state__icon" :class="icon" aria-hidden="true" />
    <h3 class="ui-empty-state__title">{{ title }}</h3>
    <p v-if="description" class="ui-empty-state__description">{{ description }}</p>
  </div>
</template>

<style scoped>
.ui-empty-state {
  display: grid;
  place-items: center;
  min-height: 8rem;
  border: 1px dashed var(--color-border-strong);
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  padding: var(--space-4);
  gap: var(--space-2);
  text-align: center;
  background: var(--color-panel-muted);
}

.ui-empty-state__icon {
  font-size: 2rem;
  color: var(--color-accent);
}

.ui-empty-state__title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.ui-empty-state__description {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--color-text-soft);
  line-height: 1.5;
}
</style>
