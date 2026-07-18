<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings, SchedulerSettings } from "../../../types/settings";
import UiSwitch from "../../ui/UiSwitch.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

function updateScheduler(patch: Partial<SchedulerSettings>) {
  emit("update:settings", {
    ...props.settings,
    scheduler: { ...props.settings.scheduler, ...patch },
  });
}

function onModeChange(mode: "traditional" | "automatic") {
  updateScheduler({ mode });
}

function onChunkStrategyChange(enabled: boolean) {
  updateScheduler({ chunkSizeStrategy: enabled ? "adaptive" : "fixed" });
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-speed-up-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.performanceTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.performanceDescription") }}</p>
    <div class="setup-step__body">
      <div class="section">
        <span class="section__label">{{ t("settings.allocationMode") }}</span>
        <div class="mode-options" role="radiogroup" :aria-label="t('settings.allocationMode')">
          <button
            type="button"
            class="mode-card"
            :class="{ 'is-selected': settings.scheduler.mode === 'traditional' }"
            role="radio"
            :aria-checked="settings.scheduler.mode === 'traditional'"
            @click="onModeChange('traditional')"
          >
            <span class="mode-card__check i-ri-check-line" aria-hidden="true" />
            <span class="mode-card__label">{{ t("tokens.traditional") }}</span>
            <span class="mode-card__hint">{{ t("settings.traditionalHint") }}</span>
          </button>
          <button
            type="button"
            class="mode-card"
            :class="{ 'is-selected': settings.scheduler.mode === 'automatic' }"
            role="radio"
            :aria-checked="settings.scheduler.mode === 'automatic'"
            @click="onModeChange('automatic')"
          >
            <span class="mode-card__check i-ri-check-line" aria-hidden="true" />
            <span class="mode-card__label">{{ t("tokens.automatic") }}</span>
            <span class="mode-card__hint">{{ t("settings.adaptiveProfileHint") }}</span>
          </button>
        </div>
      </div>

      <div class="section">
        <UiSwitch
          :model-value="settings.scheduler.chunkSizeStrategy === 'adaptive'"
          :label="t('settings.intelligentChunkAllocation')"
          @update:model-value="onChunkStrategyChange"
        />
        <p class="section__hint">{{ t("settings.intelligentChunkAllocationHint") }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup-step {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
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
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.setup-step__description {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
  max-width: 480px;
}

.setup-step__body {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: var(--space-6);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 560px;
}

.setup-step__icon {
  font-size: 2.5rem;
  color: var(--color-accent);
}

.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  text-align: left;
}

.section__label {
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
  text-align: center;
}

.section__hint {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  line-height: var(--line-height-tight);
}

.mode-options {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-3);
  width: 100%;
}

.mode-card {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
  color: var(--color-text-main);
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease,
    transform 0.2s ease,
    box-shadow 0.2s ease;
}

.mode-card:hover {
  border-color: var(--color-border-strong);
  background: var(--color-surface-muted);
  transform: translateY(-2px);
  box-shadow: var(--shadow-card-hover);
}

.mode-card:active {
  transform: scale(0.98) translateY(0);
}

.mode-card:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.mode-card.is-selected {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}

.mode-card__check {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.25rem;
  height: 1.25rem;
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-micro);
  opacity: 0;
  transform: scale(0.5);
  transition:
    opacity 0.25s ease-out,
    transform 0.25s ease-out;
}

.mode-card.is-selected .mode-card__check {
  opacity: 1;
  transform: scale(1);
}

.mode-card__label {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
}

.mode-card__hint {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  text-align: center;
  line-height: var(--line-height-tight);
}

@media (prefers-reduced-motion: reduce) {
  .mode-card {
    transition: border-color 0.2s ease, background-color 0.2s ease;
  }

  .mode-card:hover {
    transform: none;
    box-shadow: none;
  }

  .mode-card:active {
    transform: none;
  }

  .mode-card__check {
    transition: none;
  }
}
</style>
