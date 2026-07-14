<script setup lang="ts">
import UiButton from "../ui/UiButton.vue";
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { ChecksumMode } from "../../types/download";
import type { AppSettings } from "../../types/settings";

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
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.downloads") }}</p>
          <h3>{{ t("settings.downloadsTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-download-2-line" aria-hidden="true" />
      </div>
    </template>

    <p class="settings-section__summary">{{ downloadSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.defaultDownloadLocation") }}</span>
        <div class="settings-directory-field">
          <UiInput
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
        <p class="settings-field__hint">
          {{ t("settings.defaultDownloadHint") }}
        </p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.defaultRetries") }}</span>
        <UiNumberField v-model="draft.download.defaultMaxRetries" :min="0" :max="20" />
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.globalChecksum") }}</span>
        <UiSelect v-model="draft.download.defaultChecksum" :options="checksumOptions" />
        <p class="settings-field__hint">{{ t("settings.checksumHint") }}</p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.defaultUserAgent") }}</span>
        <UiInput
          v-model="draft.download.defaultUserAgent"
          type="text"
          :placeholder="defaultUserAgentPlaceholder"
        />
        <p class="settings-field__hint">{{ t("settings.defaultUserAgentHint") }}</p>
      </label>
    </div>
  </UiCard>
</template>
