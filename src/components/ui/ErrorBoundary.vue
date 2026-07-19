<script setup lang="ts">
import { onErrorCaptured, ref } from "vue";
import { useI18n } from "../../i18n";

const { t } = useI18n();

const hasError = ref(false);
const errorKey = ref(0);
const lastError = ref<string | null>(null);

onErrorCaptured((err, _instance, info) => {
  hasError.value = true;
  lastError.value = err instanceof Error ? err.message : String(err);
  console.error("[ErrorBoundary]", err, info);
  return false;
});

/**
 * Note: Retry re-mounts the child component tree with the same props/data.
 * If the crash was caused by corrupt data (not a transient error), retry will
 * fail again. The parent component should refresh its data source if retry
 * repeatedly fails.
 */
function handleRetry() {
  hasError.value = false;
  errorKey.value++;
}
</script>

<template>
  <slot v-if="!hasError" :key="errorKey" />
  <div v-else class="error-boundary" role="alert">
    <span class="error-boundary__icon i-ri-error-warning-line" aria-hidden="true" />
    <h3 class="error-boundary__title">{{ t("errorBoundary.title") }}</h3>
    <p class="error-boundary__description">{{ t("errorBoundary.description") }}</p>
    <details v-if="lastError" class="error-boundary__details">
      <summary>{{ t("errorBoundary.details") }}</summary>
      <pre class="error-boundary__message">{{ lastError }}</pre>
    </details>
    <button class="error-boundary__retry" @click="handleRetry">
      <span class="error-boundary__retry-icon i-ri-refresh-line" aria-hidden="true" />
      {{ t("errorBoundary.retry") }}
    </button>
  </div>
</template>

<style scoped>
.error-boundary {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-6);
  min-height: 8rem;
  text-align: center;
  background: var(--color-panel-muted);
  border: 1px solid var(--color-danger-border);
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
}

.error-boundary__icon {
  font-size: 2rem;
  color: var(--color-danger-text);
}

.error-boundary__title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.error-boundary__description {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--color-text-soft);
  line-height: 1.5;
  max-width: 24rem;
}

.error-boundary__retry {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: var(--space-2);
  padding: 0 0.875rem;
  min-height: 2.25rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  font: inherit;
  font-size: var(--font-size-body);
  font-weight: 600;
  cursor: pointer;
  letter-spacing: -0.01em;
  transition:
    background-color 0.2s ease,
    box-shadow 0.2s ease;
}

.error-boundary__retry:hover {
  background: var(--color-accent-strong);
}

.error-boundary__retry:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.error-boundary__retry-icon {
  font-size: 1rem;
}

.error-boundary__details {
  width: 100%;
  max-width: 24rem;
  margin-top: var(--space-2);
  font-size: var(--font-size-small);
  color: var(--color-text-soft);
  text-align: left;
}

.error-boundary__details summary {
  cursor: pointer;
  color: var(--color-text-muted);
  font-weight: 500;
}

.error-boundary__details summary:hover {
  color: var(--color-heading);
}

.error-boundary__message {
  margin: var(--space-2) 0 0;
  padding: var(--space-2);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: var(--font-size-small);
  white-space: pre-wrap;
  word-break: break-all;
  line-height: 1.4;
  overflow-x: auto;
}
</style>
