<script setup lang="ts">
import { computed } from "vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { SupportedLanguage } from "../../i18n/resources";
import type {
  AppSettings,
  BackgroundOpacityPreset,
  CloseBehavior,
  ColorMode,
  ThemeColor,
} from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";
import {
  enable as enableAutostart,
  disable as disableAutostart,
} from "@tauri-apps/plugin-autostart";

const draft = defineModel<AppSettings>("draft", { required: true });

const props = defineProps<{
  t: (key: string, options?: Record<string, unknown>) => string;
  language: SupportedLanguage;
  languageOptions: Array<{ label: string; value: string }>;
  colorModeOptions: Array<{ label: string; value: ColorMode }>;
  backgroundOpacityOptions: Array<{ label: string; value: BackgroundOpacityPreset }>;
}>();

// Autostart toggle — syncs with the plugin directly
async function onAutostartChange(value: boolean) {
  // Dev builds must not register autostart: the .desktop file would point at
  // the debug binary, which cannot run standalone after reboot (no dev
  // server) and shows a blank window.
  if (value && import.meta.env.DEV) {
    draft.value.autostart = false;
    console.warn("[autostart] registration is disabled in dev mode");
    return;
  }
  draft.value.autostart = value;
  try {
    if (value) {
      await enableAutostart();
    } else {
      await disableAutostart();
    }
  } catch {
    // ignore errors (e.g. permission denied)
  }
}

// Close behavior options
const closeBehaviorOptions = computed<Array<{ label: string; value: CloseBehavior }>>(() => [
  { label: props.t("settings.closeBehaviorMinimizeToTray"), value: "minimizeToTray" },
  { label: props.t("settings.closeBehaviorExit"), value: "exit" },
]);

const doubleClickCompletedOptions = computed<Array<{ label: string; value: string }>>(() => [
  { label: props.t("settings.doubleClickNone"), value: "none" },
  { label: props.t("settings.doubleClickOpenFile"), value: "open_file" },
  { label: props.t("settings.doubleClickOpenInExplorer"), value: "open_in_explorer" },
  { label: props.t("settings.doubleClickOpenDownloadDir"), value: "open_download_dir" },
]);

const doubleClickUncompletedOptions = computed<Array<{ label: string; value: string }>>(() => [
  { label: props.t("settings.doubleClickNone"), value: "none" },
  { label: props.t("settings.doubleClickTogglePauseResume"), value: "toggle_pause_resume" },
]);

const emit = defineEmits<{
  changeLanguage: [language: SupportedLanguage];
}>();
</script>

<template>
  <div class="appearance-panel">
    <SettingsSection :title="t('settings.languageTitle')" icon="i-ri-translate-2">
      <UiSelect
        :model-value="language"
        :options="languageOptions"
        @update:model-value="emit('changeLanguage', $event as SupportedLanguage)"
      />
    </SettingsSection>

    <SettingsSection :title="t('settings.appearanceTitle')" icon="i-ri-palette-line">
      <div class="settings-grid">
        <SettingsField :label="t('settings.colorMode')" :info-tooltip="t('settings.colorModeHint')">
          <UiSelect v-model="draft.appearance.colorMode" :options="colorModeOptions" />
        </SettingsField>

        <SettingsField :label="t('settings.themeColor')">
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
        </SettingsField>

        <SettingsField
          :label="t('settings.backgroundOpacity')"
          :info-tooltip="t('settings.backgroundOpacityHint')"
        >
          <UiSelect
            v-model="draft.appearance.backgroundOpacity"
            :options="backgroundOpacityOptions"
          />
        </SettingsField>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('settings.infoPanelTitle')" icon="i-ri-information-line">
      <div class="settings-grid">
        <SettingsField
          :label="t('settings.detailInfoPanel')"
          :info-tooltip="t('settings.detailInfoHint')"
        >
          <UiSwitch v-model="draft.appearance.showDetailInfo" />
        </SettingsField>
      </div>
    </SettingsSection>

    <SettingsSection
      :title="t('settings.notificationSettings.title')"
      icon="i-ri-notification-3-line"
      :summary="t('settings.notificationSettings.description')"
    >
      <div class="settings-grid">
        <SettingsField :label="t('settings.notificationSettings.toggleLabel')">
          <UiSwitch v-model="draft.notifications.enabled" />
        </SettingsField>
      </div>
    </SettingsSection>

    <SettingsSection :title="t('settings.startupTitle')" icon="i-ri-windows-line">
      <div class="settings-grid">
        <SettingsField :label="t('settings.autoStart')" :info-tooltip="t('settings.autoStartHint')">
          <UiSwitch :model-value="draft.autostart" @update:model-value="onAutostartChange" />
        </SettingsField>
      </div>
    </SettingsSection>

    <!-- Close Behavior -->
    <SettingsSection :title="t('settings.closeBehaviorTitle')" icon="i-ri-close-line">
      <div class="settings-grid">
        <SettingsField :label="t('settings.closeBehaviorTitle')">
          <UiSelect v-model="draft.appearance.closeBehavior" :options="closeBehaviorOptions" />
        </SettingsField>
      </div>
    </SettingsSection>

    <!-- Double-Click Behavior -->
    <SettingsSection :title="t('settings.doubleClickTitle')" icon="i-ri-cursor-line">
      <div class="settings-grid">
        <SettingsField
          :label="t('settings.doubleClickOnCompleted')"
          :info-tooltip="t('settings.doubleClickOnCompletedHint')"
        >
          <UiSelect
            v-model="draft.doubleClick.onCompleted"
            :options="doubleClickCompletedOptions"
          />
        </SettingsField>
        <SettingsField
          :label="t('settings.doubleClickOnUncompleted')"
          :info-tooltip="t('settings.doubleClickOnUncompletedHint')"
        >
          <UiSelect
            v-model="draft.doubleClick.onUncompleted"
            :options="doubleClickUncompletedOptions"
          />
        </SettingsField>
      </div>
    </SettingsSection>
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
  background: linear-gradient(135deg, #b45309, #854d0e);
  color: #ffffff;
}

.theme-color-button--sky {
  background: linear-gradient(135deg, #0369a1, #075985);
  color: #ffffff;
}

.theme-color-button--lime {
  background: linear-gradient(135deg, #4d7c0f, #3f6212);
  color: #ffffff;
}
</style>
