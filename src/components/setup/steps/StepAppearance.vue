<script setup lang="ts">
import { onBeforeUnmount } from "vue";
import { useI18n } from "../../../i18n";
import type {
  AppSettings,
  AppearanceSettings,
  BackgroundOpacityPreset,
  ColorMode,
  ThemeColor,
} from "../../../types/settings";
import UiSelect from "../../ui/UiSelect.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

const themeColorOrder: ThemeColor[] = ["lime", "amber", "sky"];

const colorModeOptions: { label: string; value: ColorMode }[] = [
  { label: t("settings.colorModeNames.system"), value: "system" },
  { label: t("settings.colorModeNames.light"), value: "light" },
  { label: t("settings.colorModeNames.dark"), value: "dark" },
];

const backgroundOpacityOptions: { label: string; value: BackgroundOpacityPreset }[] = [
  { label: t("settings.backgroundOpacityNames.default"), value: "default" },
  { label: t("settings.backgroundOpacityNames.acrylic"), value: "acrylic" },
  { label: t("settings.backgroundOpacityNames.frosted"), value: "frosted" },
];

const originalTheme = document.documentElement.dataset.theme;

onBeforeUnmount(() => {
  // Restore original theme when leaving the appearance step
  if (originalTheme) {
    document.documentElement.dataset.theme = originalTheme;
  } else {
    delete document.documentElement.dataset.theme;
  }
});

function updateAppearance(patch: Partial<AppearanceSettings>) {
  emit("update:settings", {
    ...props.settings,
    appearance: { ...props.settings.appearance, ...patch },
  });
}

function selectThemeColor(themeColor: ThemeColor) {
  // Live-preview by setting the theme on document root so :root[data-theme] CSS rules apply
  document.documentElement.dataset.theme = themeColor;
  updateAppearance({ themeColor });
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-palette-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.appearanceTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.appearanceDescription") }}</p>
    <div class="setup-step__body">
      <div class="appearance-grid">
        <div class="field-group">
          <label class="field-label">{{ t("setupWizard.colorModeLabel") }}</label>
          <UiSelect
            :model-value="settings.appearance.colorMode"
            :options="colorModeOptions"
            @update:model-value="updateAppearance({ colorMode: $event })"
          />
        </div>

        <div class="field-group">
          <label class="field-label">{{ t("setupWizard.themeColorLabel") }}</label>
          <div class="theme-color-options">
            <button
              v-for="color in themeColorOrder"
              :key="color"
              type="button"
              class="theme-color-button"
              :class="[
                `theme-color-button--${color}`,
                { 'is-active': settings.appearance.themeColor === color },
              ]"
              :aria-label="t(`settings.themeColorNames.${color}`)"
              :aria-pressed="settings.appearance.themeColor === color"
              @click="selectThemeColor(color)"
            >
              <span
                v-if="settings.appearance.themeColor === color"
                class="i-ri-check-line"
                aria-hidden="true"
              />
            </button>
          </div>
        </div>

        <div class="field-group">
          <label class="field-label">{{ t("setupWizard.backgroundOpacityLabel") }}</label>
          <UiSelect
            :model-value="settings.appearance.backgroundOpacity"
            :options="backgroundOpacityOptions"
            @update:model-value="updateAppearance({ backgroundOpacity: $event })"
          />
        </div>
      </div>

      <div
        class="theme-preview"
        :data-theme="settings.appearance.themeColor"
      >
        <div class="theme-preview__panel">
          <span class="theme-preview__dot" aria-hidden="true" />
          <div class="theme-preview__lines">
            <span class="theme-preview__line" />
            <span class="theme-preview__line theme-preview__line--short" />
          </div>
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
  gap: var(--space-5);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 560px;
}

.setup-step__icon {
  font-size: 2.5rem;
  color: var(--color-accent);
}

.appearance-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: var(--space-4);
  text-align: left;
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.field-label {
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
}

.theme-color-options {
  display: flex;
  gap: var(--space-3);
  align-items: center;
  margin-top: var(--space-2);
}

.theme-preview {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
}

.theme-preview__panel {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  max-width: 240px;
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: var(--border-width-thin) solid var(--color-border);
}

.theme-preview__dot {
  width: 2rem;
  height: 2rem;
  border-radius: var(--radius-md);
  background: var(--color-accent);
  flex-shrink: 0;
}

.theme-preview__lines {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1;
}

.theme-preview__line {
  height: 0.5rem;
  border-radius: var(--radius-sm);
  background: var(--color-accent-soft);
}

.theme-preview__line--short {
  width: 60%;
}

.theme-preview,
.theme-preview__panel,
.theme-preview__dot,
.theme-preview__line {
  transition:
    background-color 0.3s ease,
    border-color 0.3s ease;
}

.theme-color-button {
  transition:
    transform 0.2s ease,
    box-shadow 0.2s ease;
}

.theme-color-button:hover {
  transform: scale(1.1);
}

.theme-color-button:active {
  transform: scale(0.95);
}

.theme-color-button .i-ri-check-line {
  opacity: 0;
  transform: scale(0.5);
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.theme-color-button.is-active .i-ri-check-line {
  opacity: 1;
  transform: scale(1);
}

@media (prefers-reduced-motion: reduce) {
  .theme-preview,
  .theme-preview__panel,
  .theme-preview__dot,
  .theme-preview__line {
    transition: none;
  }

  .theme-color-button {
    transition: box-shadow var(--duration-fast);
  }

  .theme-color-button .i-ri-check-line {
    transition: none;
  }
}
</style>
