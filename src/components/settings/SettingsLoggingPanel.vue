<script setup lang="ts">
import { computed } from "vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings, LogLevel } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  logLevelOptions: Array<{ label: string; value: LogLevel }>;
  loggingSummary: string;
}>();

type RetentionStrategy = "none" | "count" | "days" | "both";

const retentionStrategy = computed({
  get(): RetentionStrategy {
    const hasCount = props.draft.logging.retentionCount != null;
    const hasDays = props.draft.logging.retentionDays != null;
    if (hasCount && hasDays) return "both";
    if (hasCount) return "count";
    if (hasDays) return "days";
    return "none";
  },
  set(value: RetentionStrategy) {
    if (value === "none") {
      props.draft.logging.retentionCount = null;
      props.draft.logging.retentionDays = null;
    } else if (value === "count") {
      props.draft.logging.retentionDays = null;
      if (props.draft.logging.retentionCount == null) {
        props.draft.logging.retentionCount = 10;
      }
    } else if (value === "days") {
      props.draft.logging.retentionCount = null;
      if (props.draft.logging.retentionDays == null) {
        props.draft.logging.retentionDays = 30;
      }
    } else if (value === "both") {
      if (props.draft.logging.retentionCount == null) {
        props.draft.logging.retentionCount = 10;
      }
      if (props.draft.logging.retentionDays == null) {
        props.draft.logging.retentionDays = 30;
      }
    }
  },
});

const retentionStrategyOptions = computed(() => [
  { label: props.t("settings.loggingRetentionNone"), value: "none" as const },
  { label: props.t("settings.loggingRetentionCount"), value: "count" as const },
  { label: props.t("settings.loggingRetentionDays"), value: "days" as const },
  { label: props.t("settings.loggingRetentionBoth"), value: "both" as const },
]);
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

      <SettingsField :label="t('settings.loggingRetentionStrategy')">
        <UiSelect v-model="retentionStrategy" :options="retentionStrategyOptions" />
      </SettingsField>

      <SettingsField
        v-if="retentionStrategy === 'count' || retentionStrategy === 'both'"
        :label="t('settings.loggingRetentionCountField')"
      >
        <div class="field-with-unit">
          <UiTextField v-model="draft.logging.retentionCount" type="number" :min="0" :max="1000" />
          <span class="field-unit">{{ t("settings.loggingRetentionCountUnit") }}</span>
        </div>
      </SettingsField>

      <SettingsField
        v-if="retentionStrategy === 'days' || retentionStrategy === 'both'"
        :label="t('settings.loggingRetentionDaysField')"
      >
        <div class="field-with-unit">
          <UiTextField v-model="draft.logging.retentionDays" type="number" :min="0" :max="3650" />
          <span class="field-unit">{{ t("settings.loggingRetentionDaysUnit") }}</span>
        </div>
      </SettingsField>
    </div>
  </SettingsSection>
</template>

<style scoped>
.field-with-unit {
  display: flex;
  align-items: center;
}

.field-unit {
  color: var(--color-text-muted);
  font-size: 0.9em;
  margin-left: 0.5ch;
}
</style>
