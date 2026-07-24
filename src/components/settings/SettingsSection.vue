<script setup lang="ts">
defineProps<{
  title?: string;
  icon?: string;
  summary?: string;
}>();
</script>

<template>
  <section class="settings-section">
    <div v-if="title || icon" class="settings-section__head">
      <div>
        <h3 v-if="title">{{ title }}</h3>
      </div>
      <span v-if="icon" :class="icon" aria-hidden="true" />
    </div>
    <p v-if="summary" class="settings-section__summary">{{ summary }}</p>
    <div class="settings-section__body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.settings-section {
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  padding: var(--space-5, 1.25rem);
  box-shadow: var(--shadow-card, 0 1px 3px rgba(0, 0, 0, 0.08));
  overflow: hidden;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.settings-section:hover {
  border-color: var(--color-border-strong);
  box-shadow: var(--shadow-card-hover);
}

.settings-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.settings-section__head h3 {
  margin: 0.2rem 0 0;
  color: var(--color-heading);
  font-size: 1rem;
}

.settings-section__head > span[aria-hidden] {
  flex-shrink: 0;
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
}

.settings-section__summary {
  margin: 0;
  margin-top: var(--space-3);
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
}

.settings-section__body {
  display: contents;
}
</style>
