<script setup lang="ts">
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings, LogLevel } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  logLevelOptions: Array<{ label: string; value: LogLevel }>;
  loggingSummary: string;
}>();
</script>

<template>
  <SettingsSection
    :title="t('settings.loggingTitle')"
    icon="i-ri-file-list-3-line"
    :summary="loggingSummary"
  >
    <div class="settings-grid">
      <SettingsField :label="t('settings.loggingEnabled')">
        <UiSwitch v-model="draft.logging.enabled" :label="t('settings.loggingToggleText')" />
      </SettingsField>

      <SettingsField :label="t('settings.loggingLevel')">
        <UiSelect v-model="draft.logging.level" :options="logLevelOptions" />
      </SettingsField>

      <SettingsField
        wide
        :label="t('settings.loggingPath')"
        :info-tooltip="t('settings.loggingPathHint')"
      >
        <UiTextField
          v-model="draft.logging.filePath"
          type="text"
          :placeholder="t('settings.loggingPathPlaceholder')"
        />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
