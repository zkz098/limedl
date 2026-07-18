<script setup lang="ts">
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { AppSettings, ProxyMode } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  proxyModeOptions: Array<{ label: string; value: ProxyMode }>;
  proxySummary: string;
}>();
</script>

<template>
  <SettingsSection
    :title="t('settings.proxyTitle')"
    icon="i-ri-global-line"
    :summary="proxySummary"
  >
    <div class="settings-grid">
      <SettingsField :label="t('settings.proxyMode')">
        <UiSelect v-model="draft.proxy.mode" :options="proxyModeOptions" />
      </SettingsField>

      <SettingsField
        v-if="draft.proxy.mode === 'manual'"
        wide
        :label="t('settings.proxyAddress')"
        :info-tooltip="t('settings.proxyHint')"
      >
        <UiTextField
          v-model="draft.proxy.manualUrl"
          type="text"
          placeholder="http://127.0.0.1:7890"
        />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
