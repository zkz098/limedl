<script setup lang="ts">
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings, LogLevel } from "../../types/settings";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  logLevelOptions: Array<{ label: string; value: LogLevel }>;
  loggingSummary: string;
}>();
</script>

<template>
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <h3>{{ t("settings.loggingTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-file-list-3-line" aria-hidden="true" />
      </div>
    </template>

    <p class="settings-section__summary">{{ loggingSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.loggingEnabled") }}</span>
        <UiSwitch v-model="draft.logging.enabled" :label="t('settings.loggingToggleText')" />
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.loggingLevel") }}</span>
        <UiSelect v-model="draft.logging.level" :options="logLevelOptions" />
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.loggingPath") }}</span>
        <UiInput
          v-model="draft.logging.filePath"
          type="text"
          :placeholder="t('settings.loggingPathPlaceholder')"
        />
        <p class="settings-field__hint">{{ t("settings.loggingPathHint") }}</p>
      </label>
    </div>
  </UiCard>
</template>
