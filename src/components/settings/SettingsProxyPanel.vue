<script setup lang="ts">
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { AppSettings, ProxyMode } from "../../types/settings";

defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  proxyModeOptions: Array<{ label: string; value: ProxyMode }>;
  proxySummary: string;
}>();
</script>

<template>
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <h3>{{ t("settings.proxyTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-global-line" aria-hidden="true" />
      </div>
    </template>

    <p class="settings-section__summary">{{ proxySummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.proxyMode") }}</span>
        <UiSelect v-model="draft.proxy.mode" :options="proxyModeOptions" />
      </label>

      <label v-if="draft.proxy.mode === 'manual'" class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.proxyAddress") }}</span>
        <UiInput v-model="draft.proxy.manualUrl" type="text" placeholder="http://127.0.0.1:7890" />
        <p class="settings-field__hint">
          {{ t("settings.proxyHint") }}
        </p>
      </label>
    </div>
  </UiCard>
</template>
