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
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
import SettingsField from "../../settings/SettingsField.vue";
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
  document.documentElement.dataset.theme = themeColor;
  updateAppearance({ themeColor });
}
</script>

<template>
  <StepShell
    icon="i-ri-palette-line"
    title-key="setupWizard.appearanceTitle"
    description-key="setupWizard.appearanceDescription"
  >
    <SettingsSection :title="t('setupWizard.appearanceTitle')" icon="i-ri-brush-line">
      <div class="appearance-grid">
        <SettingsField :label="t('setupWizard.colorModeLabel')">
          <UiSelect
            :model-value="settings.appearance.colorMode"
            :options="colorModeOptions"
            @update:model-value="updateAppearance({ colorMode: $event })"
          />
        </SettingsField>

        <SettingsField :label="t('setupWizard.themeColorLabel')">
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
        </SettingsField>

        <SettingsField :label="t('setupWizard.backgroundOpacityLabel')">
          <UiSelect
            :model-value="settings.appearance.backgroundOpacity"
            :options="backgroundOpacityOptions"
            @update:model-value="updateAppearance({ backgroundOpacity: $event })"
          />
        </SettingsField>
      </div>

      <div class="theme-preview" :data-theme="settings.appearance.themeColor">
        <div class="theme-preview__panel">
          <span class="theme-preview__dot" aria-hidden="true" />
          <div class="theme-preview__lines">
            <span class="theme-preview__line" />
            <span class="theme-preview__line theme-preview__line--short" />
          </div>
        </div>
      </div>
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
.appearance-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: var(--space-4);
  text-align: left;
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
  max-width: 15rem;
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

  .theme-color-button .i-ri-check-line {
    transition: none;
  }
}
</style>
