<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings, CdnAccelerationSettings } from "../../../types/settings";
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
import SettingsField from "../../settings/SettingsField.vue";
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

function onCdnEnabledChange(enabled: boolean) {
  updateCdn({ enabled });
}
</script>

<template>
  <StepShell
    icon="i-ri-flashlight-fill"
    title-key="setupWizard.cdnTitle"
    description-key="setupWizard.cdnDescription"
  >
    <SettingsSection
      :title="t('setupWizard.cdnTitle')"
      icon="i-ri-speed-line"
      :summary="t('setupWizard.cdnDescription')"
    >
      <SettingsField>
        <UiSwitch
          :model-value="settings.cdnAcceleration.enabled"
          :label="t('setupWizard.cdnEnableLabel')"
          @update:model-value="onCdnEnabledChange"
        />
      </SettingsField>

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
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
.recommendation-card {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-info-border);
  border-radius: var(--radius-md);
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
  border-radius: var(--radius-md);
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
