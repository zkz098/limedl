<script setup lang="ts">
import UiButton from "../ui/UiButton.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { ChecksumMode } from "../../types/download";
import type { AppSettings } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  checksumOptions: Array<{ label: string; value: ChecksumMode }>;
  downloadSummary: string;
  isPickingDirectory: boolean;
  defaultUserAgentPlaceholder: string;
}>();

const emit = defineEmits<{
  pickDirectory: [];
}>();
</script>

<template>
  <SettingsSection :title="t('settings.downloadsTitle')" icon="i-ri-download-2-line" :summary="downloadSummary">
    <div class="settings-grid">
      <SettingsField wide :label="t('settings.defaultDownloadLocation')" :hint="t('settings.defaultDownloadHint')">
        <div class="settings-directory-field">
          <UiTextField
            v-model="draft.download.defaultDownloadDir"
            type="text"
            :placeholder="t('settings.defaultDownloadPlaceholder')"
          />
          <UiButton
            type="button"
            variant="secondary"
            size="sm"
            :loading="isPickingDirectory"
            @click="emit('pickDirectory')"
          >
            {{ isPickingDirectory ? t("common.browsing") : t("common.browse") }}
          </UiButton>
        </div>
      </SettingsField>

      <SettingsField :label="t('settings.defaultRetries')">
        <UiTextField type="number" v-model="draft.download.defaultMaxRetries" :min="0" :max="20" />
      </SettingsField>

      <SettingsField :label="t('settings.globalChecksum')" :hint="t('settings.checksumHint')">
        <UiSelect v-model="draft.download.defaultChecksum" :options="checksumOptions" />
      </SettingsField>

      <SettingsField wide :label="t('settings.defaultUserAgent')" :hint="t('settings.defaultUserAgentHint')">
        <UiTextField
          v-model="draft.download.defaultUserAgent"
          type="text"
          :placeholder="defaultUserAgentPlaceholder"
        />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
