<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings } from "../../../types/settings";
import logoUrl from "../../../assets/logo.png";
import SettingsSection from "../../settings/SettingsSection.vue";

const { t } = useI18n();

defineProps<{
  settings: AppSettings;
}>();

defineEmits<{
  "update:settings": [settings: AppSettings];
}>();
</script>

<template>
  <div class="step-welcome">
    <SettingsSection :title="t('setupWizard.welcomeTitle')" icon="i-ri-home-smile-line">
      <div class="step-welcome__identity">
        <img :src="logoUrl" class="step-welcome__logo" alt="Limedl" />
        <div class="step-welcome__meta">
          <h1 class="step-welcome__title">{{ t("setupWizard.welcomeTitle") }}</h1>
          <p class="step-welcome__subtitle">{{ t("setupWizard.welcomeSubtitle") }}</p>
        </div>
      </div>

      <div class="step-welcome__license">
        <span class="i-ri-scales-line step-welcome__license-icon" aria-hidden="true" />
        <p class="step-welcome__license-text">{{ t("setupWizard.welcomeLicense") }}</p>
      </div>
    </SettingsSection>
  </div>
</template>

<style scoped>
.step-welcome {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  min-height: 0;
  padding: var(--space-6);
}

.step-welcome :deep(.settings-section) {
  max-width: 32rem;
  width: 100%;
}

.step-welcome__identity {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin-bottom: var(--space-5);
}

.step-welcome__logo {
  width: 4rem;
  height: 4rem;
  flex-shrink: 0;
  border-radius: var(--radius-lg);
  background: var(--color-surface-muted);
  object-fit: contain;
  box-shadow: var(--shadow-card);
}

.step-welcome__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  text-align: left;
}

.step-welcome__title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
  line-height: var(--line-height-display);
}

.step-welcome__subtitle {
  margin: 0;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
}

.step-welcome__license {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  text-align: left;
}

.step-welcome__license-icon {
  flex-shrink: 0;
  margin-top: var(--space-1);
  font-size: var(--font-size-metric);
  color: var(--color-text-muted);
}

.step-welcome__license-text {
  margin: 0;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
}

@media (max-width: 680px) {
  .step-welcome {
    padding: var(--space-4);
  }

  .step-welcome__identity {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
