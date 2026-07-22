<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings } from "../../../types/settings";
import logoUrl from "../../../assets/logo.png";

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
    <div class="step-welcome__hero">
      <img :src="logoUrl" class="step-welcome__logo" alt="Limedl" />
      <h1 class="step-welcome__title">{{ t("setupWizard.welcomeTitle") }}</h1>
      <p class="step-welcome__subtitle">{{ t("setupWizard.welcomeSubtitle") }}</p>
      <p class="step-welcome__license">{{ t("setupWizard.welcomeLicense") }}</p>
    </div>
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
  text-align: center;
  padding: var(--space-6);
}

.step-welcome__hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
  max-width: 32rem;
}

.step-welcome__logo {
  width: 5rem;
  height: 5rem;
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
}

.step-welcome__title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.step-welcome__subtitle {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
}

.step-welcome__license {
  margin: var(--space-2) 0 0;
  font-size: var(--font-size-small);
  color: var(--color-text-soft);
}
</style>
