<script setup lang="ts">
import { useI18n } from "../../i18n";
import type { SetupStep } from "../../composables/useSetupWizard";

const props = defineProps<{
  steps: SetupStep[];
  currentStepIndex: number;
  completedStepIndices: Set<number>;
}>();

const emit = defineEmits<{
  "select-step": [index: number];
}>();

const { t } = useI18n();

function isCompleted(index: number) {
  return props.completedStepIndices.has(index);
}

function isCurrent(index: number) {
  return index === props.currentStepIndex;
}

function isClickable(index: number) {
  return isCompleted(index) || index === props.currentStepIndex;
}

function handleClick(index: number) {
  if (!isClickable(index)) {
    return;
  }
  emit("select-step", index);
}
</script>

<template>
  <nav class="setup-step-indicator" :aria-label="t('setupWizard.stepsLabel')">
    <div class="setup-step-indicator__header">
      <span class="setup-step-indicator__count">
        {{ t("setupWizard.stepIndicator", { current: currentStepIndex + 1, total: steps.length }) }}
      </span>
    </div>
    <ol class="setup-step-indicator__list">
      <li
        v-for="(step, index) in steps"
        :key="step.id"
        class="setup-step-indicator__item"
        :class="{
          'is-current': isCurrent(index),
          'is-completed': isCompleted(index),
          'is-future': !isCurrent(index) && !isCompleted(index),
        }"
      >
        <button
          type="button"
          class="setup-step-indicator__button"
          :class="{ 'is-clickable': isClickable(index) }"
          :disabled="!isClickable(index)"
          :aria-current="isCurrent(index) ? 'step' : undefined"
          @click="handleClick(index)"
        >
          <span class="setup-step-indicator__badge" aria-hidden="true">
            <span v-if="isCompleted(index)" class="i-ri-check-line" aria-hidden="true" />
            <span v-else :class="step.icon" aria-hidden="true" />
          </span>
          <span class="setup-step-indicator__label">{{ t(step.labelKey) }}</span>
        </button>
      </li>
    </ol>
  </nav>
</template>

<style scoped>
.setup-step-indicator {
  display: flex;
  flex-direction: column;
  width: 12rem;
  height: 100%;
  background: var(--color-panel);
  border-right: var(--border-width-thin) solid var(--color-border);
  padding: var(--space-5) var(--space-4);
  gap: var(--space-4);
}

.setup-step-indicator__header {
  display: flex;
  align-items: center;
  justify-content: center;
}

.setup-step-indicator__count {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

.setup-step-indicator__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.setup-step-indicator__item {
  display: flex;
  flex-direction: column;
}

.setup-step-indicator__button {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: var(--border-width-thin) solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  font: inherit;
  font-size: var(--font-size-small);
  text-align: left;
  cursor: default;
  transition:
    background-color 0.2s ease,
    color 0.2s ease,
    border-color 0.2s ease;
}

.setup-step-indicator__button.is-clickable {
  cursor: pointer;
}

.setup-step-indicator__button.is-clickable:hover:not(:disabled) {
  background: var(--color-surface-muted);
  color: var(--color-text-main);
}

.setup-step-indicator__button:focus-visible {
  outline: none;
  border-color: var(--color-focus-ring);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.setup-step-indicator__button:disabled {
  opacity: 1;
}

.setup-step-indicator__button.is-clickable:hover:not(:disabled) .setup-step-indicator__badge {
  transform: scale(1.05);
}

.setup-step-indicator__item.is-current .setup-step-indicator__button {
  background: var(--color-accent-soft);
  border-color: var(--color-accent-soft-border);
  color: var(--color-accent-strong);
}

.setup-step-indicator__item.is-completed .setup-step-indicator__button {
  color: var(--color-text-main);
}

.setup-step-indicator__item.is-future .setup-step-indicator__button {
  opacity: 0.55;
}

.setup-step-indicator__badge {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 1.625rem;
  height: 1.625rem;
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: var(--border-width-thin) solid var(--color-border);
  font-size: var(--font-size-small);
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    transform 0.2s ease,
    box-shadow 0.2s ease;
}

.setup-step-indicator__item.is-current .setup-step-indicator__badge {
  background: var(--color-accent);
  border-color: var(--color-accent);
  color: var(--color-accent-contrast);
  animation: current-pulse 2.8s ease-in-out infinite;
}

.setup-step-indicator__item.is-completed .setup-step-indicator__badge {
  background: var(--color-success-bg);
  border-color: var(--color-success-border);
  color: var(--color-success-text);
}

.setup-step-indicator__item.is-completed .setup-step-indicator__badge .i-ri-check-line {
  animation: checkmark-pop 0.35s ease-out;
}

@keyframes checkmark-pop {
  0% {
    opacity: 0;
    transform: scale(0.4) rotate(-12deg);
  }
  60% {
    transform: scale(1.15) rotate(2deg);
  }
  100% {
    opacity: 1;
    transform: scale(1) rotate(0);
  }
}

@keyframes current-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 0 var(--color-accent-soft-border);
  }
  50% {
    box-shadow: 0 0 0 0.2rem var(--color-accent-soft-border);
  }
}

.setup-step-indicator__label {
  flex: 1;
  min-width: 0;
  font-weight: var(--font-weight-semibold);
}

@media (max-width: 680px) {
  .setup-step-indicator {
    flex-direction: row;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: auto;
    min-height: 3.5rem;
    padding: var(--space-3) var(--space-4);
    border-right: none;
    border-bottom: var(--border-width-thin) solid var(--color-border);
    gap: var(--space-3);
  }

  .setup-step-indicator__header {
    display: none;
  }

  .setup-step-indicator__list {
    flex-direction: row;
    justify-content: center;
    gap: var(--space-2);
    width: 100%;
  }

  .setup-step-indicator__button {
    justify-content: center;
    padding: var(--space-2);
  }

  .setup-step-indicator__label {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .setup-step-indicator__item.is-current .setup-step-indicator__badge {
    animation: none;
  }

  .setup-step-indicator__item.is-completed .setup-step-indicator__badge .i-ri-check-line {
    animation: none;
  }
}
</style>
