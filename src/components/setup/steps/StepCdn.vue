<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings, CdnAccelerationSettings } from "../../../types/settings";
import UiSwitch from "../../ui/UiSwitch.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

const isZh = navigator.language.toLowerCase().startsWith("zh");

function updateCdn(patch: Partial<CdnAccelerationSettings>) {
  emit("update:settings", {
    ...props.settings,
    cdnAcceleration: { ...props.settings.cdnAcceleration, ...patch },
  });
}

function onEnabledChange(enabled: boolean) {
  updateCdn({ enabled });
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-flashlight-fill" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.cdnTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.cdnDescription") }}</p>
    <div class="setup-step__body">
      <div class="cdn-control">
        <UiSwitch
          :model-value="settings.cdnAcceleration.enabled"
          :label="t('setupWizard.cdnEnableLabel')"
          @update:model-value="onEnabledChange"
        />
      </div>

      <div class="recommendation-card">
        <span class="recommendation-card__icon i-ri-lightbulb-line" aria-hidden="true" />
        <p class="recommendation-card__text">
          {{ isZh ? t("setupWizard.cdnRecommendZh") : t("setupWizard.cdnRecommendDefault") }}
        </p>
      </div>

      <div v-if="settings.cdnAcceleration.activeIp" class="status-card">
        <span class="status-card__icon i-ri-wifi-line" aria-hidden="true" />
        <div class="status-card__content">
          <span class="status-card__label">{{ t("setupWizard.cdnActiveIp") }}</span>
          <span class="status-card__value">{{ settings.cdnAcceleration.activeIp }}</span>
        </div>
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
  gap: var(--space-4);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 560px;
}

.setup-step__icon {
  font-size: 2.5rem;
  color: var(--color-accent);
}

.cdn-control {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
}

.recommendation-card {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-info-border);
  border-radius: var(--radius-lg);
  background: var(--color-info-bg);
  color: var(--color-info-text);
  text-align: left;
}

.recommendation-card__icon {
  flex-shrink: 0;
  font-size: 1.25rem;
  margin-top: 0.125rem;
}

.recommendation-card__text {
  margin: 0;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
}

.status-card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border: var(--border-width-thin) solid var(--color-success-border);
  border-radius: var(--radius-lg);
  background: var(--color-success-bg);
  color: var(--color-success-text);
  text-align: left;
}

.status-card__icon {
  flex-shrink: 0;
  font-size: 1.25rem;
}

.status-card__content {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.status-card__label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.status-card__value {
  font-family: var(--font-mono);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
}
</style>
