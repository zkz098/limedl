<script setup lang="ts">
import { useI18n } from "../../i18n";

defineProps<{
  icon: string;
  titleKey: string;
  descriptionKey: string;
}>();

const { t } = useI18n();
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon" :class="icon" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t(titleKey) }}</h2>
    </div>
    <p class="setup-step__description">{{ t(descriptionKey) }}</p>
    <div class="setup-step__body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.setup-step {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding: var(--space-6);
  flex: 1;
  min-height: 0;
  align-items: center;
  text-align: center;
}

.setup-step__header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}

.setup-step__title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-metric);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.setup-step__description {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
  max-width: 36rem;
}

.setup-step__body {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: var(--space-5);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 48rem;
  margin-top: calc(-1 * var(--space-2));
}

.setup-step__icon {
  font-size: 2rem;
  color: var(--color-accent);
}

@media (max-width: 680px) {
  .setup-step {
    padding: var(--space-4);
  }

  .setup-step__title {
    font-size: var(--font-size-metric);
  }

  .setup-step__description {
    font-size: var(--font-size-small);
  }
}
</style>
