<script setup lang="ts">
import UiSelect from "../ui/UiSelect.vue";
import type { SupportedLanguage } from "../../i18n/resources";
import type {
  AppSettings,
  BackgroundOpacityPreset,
  ColorMode,
  ThemeColor,
} from "../../types/settings";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  language: SupportedLanguage;
  languageOptions: Array<{ label: string; value: string }>;
  colorModeOptions: Array<{ label: string; value: ColorMode }>;
  backgroundOpacityOptions: Array<{ label: string; value: BackgroundOpacityPreset }>;
}>();

const emit = defineEmits<{
  changeLanguage: [language: SupportedLanguage];
}>();
</script>

<template>
  <div class="appearance-panel">
    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("language.label") }}</p>
          <h3>{{ t("settings.languageTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-translate-2" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("language.label") }}</span>
          <UiSelect
            :model-value="language"
            :options="languageOptions"
            @update:model-value="emit('changeLanguage', $event as SupportedLanguage)"
          />
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.appearanceKicker") }}</p>
          <h3>{{ t("settings.appearanceTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-palette-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.colorMode") }}</span>
          <UiSelect v-model="draft.appearance.colorMode" :options="colorModeOptions" />
          <p class="settings-field__hint">{{ t("settings.colorModeHint") }}</p>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.themeColor") }}</span>
          <div class="theme-color-options">
            <button
              v-for="color in ['amber', 'sky', 'lime'] as ThemeColor[]"
              :key="color"
              type="button"
              class="theme-color-button"
              :class="[
                'theme-color-button--' + color,
                { 'is-active': draft.appearance.themeColor === color },
              ]"
              :aria-label="t(`settings.themeColorNames.${color}`)"
              @click="draft.appearance.themeColor = color"
            >
              <span
                v-if="draft.appearance.themeColor === color"
                class="i-ri-check-line"
                aria-hidden="true"
              />
            </button>
          </div>
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.backgroundOpacity") }}</span>
          <UiSelect
            v-model="draft.appearance.backgroundOpacity"
            :options="backgroundOpacityOptions"
          />
          <p class="settings-field__hint">{{ t("settings.backgroundOpacityHint") }}</p>
        </label>
      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.infoPanelKicker") }}</p>
          <h3>{{ t("settings.infoPanelTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-information-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.detailInfo") }}</span>
          <button
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': draft.appearance.showDetailInfo }"
            :aria-pressed="draft.appearance.showDetailInfo"
            @click="draft.appearance.showDetailInfo = !draft.appearance.showDetailInfo"
          >
            <span
              class="settings-toggle__icon"
              :class="
                draft.appearance.showDetailInfo
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">{{ t("settings.detailInfoPanel") }}</span>
          </button>
          <p class="settings-field__hint">{{ t("settings.detailInfoHint") }}</p>
        </label>


      </div>
    </section>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.notificationSettings.title") }}</p>
          <h3>{{ t("settings.notificationSettings.title") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-notification-3-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{
            t("settings.notificationSettings.enabled")
          }}</span>
          <button
            type="button"
            class="settings-toggle"
            :class="{ 'settings-toggle--active': draft.notifications.enabled }"
            :aria-pressed="draft.notifications.enabled"
            @click="draft.notifications.enabled = !draft.notifications.enabled"
          >
            <span
              class="settings-toggle__icon"
              :class="
                draft.notifications.enabled
                  ? 'i-ri-checkbox-circle-fill'
                  : 'i-ri-checkbox-blank-circle-line'
              "
              aria-hidden="true"
            />
            <span class="settings-toggle__text">{{
              t("settings.notificationSettings.title")
            }}</span>
          </button>
          <p class="settings-field__hint">{{ t("settings.notificationSettings.description") }}</p>
        </label>
      </div>
    </section>
  </div>
</template>

<style scoped>
.appearance-panel {
  display: grid;
  gap: 1rem;
}

.theme-color-options {
  display: flex;
  flex-wrap: wrap;
  gap: 0.65rem;
}

.theme-color-button {
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 2px solid transparent;
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  color: var(--color-text-soft);
  cursor: pointer;
  font-size: 1rem;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.theme-color-button:hover {
  border-color: var(--color-border-strong);
}

.theme-color-button:focus-visible {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.theme-color-button.is-active {
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.theme-color-button--amber {
  background: linear-gradient(135deg, #d97706, #eab308);
  color: #ffffff;
}

.theme-color-button--sky {
  background: linear-gradient(135deg, #0284c7, #38bdf8);
  color: #ffffff;
}

.theme-color-button--lime {
  background: linear-gradient(135deg, #4d7c0f, #a3e635);
  color: #ffffff;
}
</style>
