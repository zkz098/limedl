<script setup lang="ts">
import UiInput from "../ui/UiInput.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { AppSettings, LogLevel } from "../../types/settings";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  logLevelOptions: Array<{ label: string; value: LogLevel }>;
  loggingSummary: string;
}>();
</script>

<template>
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.logging") }}</p>
        <h3>{{ t("settings.loggingTitle") }}</h3>
      </div>
      <span class="settings-section__icon i-ri-file-list-3-line" aria-hidden="true" />
    </div>

    <p class="settings-section__summary">{{ loggingSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.loggingEnabled") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.logging.enabled }"
          :aria-pressed="draft.logging.enabled"
          @click="draft.logging.enabled = !draft.logging.enabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.logging.enabled
                ? 'i-ri-checkbox-circle-fill'
                : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.loggingToggleText") }}</span>
        </button>
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
  </section>
</template>
