<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { SupportedLanguage } from "../../../i18n/resources";
import type { AppSettings } from "../../../types/settings";

const { t, language, setLanguage } = useI18n();

defineProps<{
  settings: AppSettings;
}>();

defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

function selectLanguage(value: SupportedLanguage) {
  void setLanguage(value);
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-translate-2" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.languageTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.languageDescription") }}</p>
    <div class="setup-step__body">
      <div class="language-options" role="radiogroup" :aria-label="t('setupWizard.languageTitle')">
        <button
          type="button"
          class="language-card"
          :class="{ 'is-selected': language === 'zh-CN' }"
          role="radio"
          :aria-checked="language === 'zh-CN'"
          @click="selectLanguage('zh-CN')"
        >
          <span class="language-card__check i-ri-check-line" aria-hidden="true" />
          <span class="language-card__native">简体中文</span>
          <span class="language-card__translated">{{ t("language.zhCN") }}</span>
        </button>
        <button
          type="button"
          class="language-card"
          :class="{ 'is-selected': language === 'en-US' }"
          role="radio"
          :aria-checked="language === 'en-US'"
          @click="selectLanguage('en-US')"
        >
          <span class="language-card__check i-ri-check-line" aria-hidden="true" />
          <span class="language-card__native">English</span>
          <span class="language-card__translated">{{ t("language.enUS") }}</span>
        </button>
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
  align-items: center;
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

.language-options {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-3);
  width: 100%;
  max-width: 480px;
}

.language-card {
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

.language-card:hover {
  border-color: var(--color-border-strong);
  background: var(--color-surface-muted);
  transform: translateY(-2px);
  box-shadow: var(--shadow-card-hover);
}

.language-card:active {
  transform: scale(0.98) translateY(0);
}

.language-card:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.language-card.is-selected {
  border-color: var(--color-accent);
  background: var(--color-accent-soft);
}

.language-card__check {
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

.language-card.is-selected .language-card__check {
  opacity: 1;
  transform: scale(1);
}

.language-card__native {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
}

.language-card__translated {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

@media (prefers-reduced-motion: reduce) {
  .language-card {
    transition: border-color 0.2s ease, background-color 0.2s ease;
  }

  .language-card:hover {
    transform: none;
    box-shadow: none;
  }

  .language-card:active {
    transform: none;
  }

  .language-card__check {
    transition: none;
  }
}
</style>
