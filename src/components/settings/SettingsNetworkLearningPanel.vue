<script setup lang="ts">
import { computed } from "vue";
import UiSelect from "../ui/UiSelect.vue";
import type { AppSettings, DeviceLearningMode, NetworkSceneProfile } from "../../types/settings";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  deviceModeOptions: Array<{ label: string; value: DeviceLearningMode }>;
  networkLearningSummary: string;
  networkMetricsCards: Array<{ label: string; value: string }>;
}>();

const currentScene = computed<NetworkSceneProfile | null>(() => {
  return props.draft.networkLearning.scenes[0] ?? null;
});
</script>

<template>
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.networkLearning") }}</p>
        <h3>{{ t("settings.networkLearningTitle") }}</h3>
      </div>
      <span class="settings-section__icon i-ri-radar-line" aria-hidden="true" />
    </div>

    <p class="settings-section__summary">{{ networkLearningSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.deviceMode") }}</span>
        <UiSelect v-model="draft.networkLearning.deviceMode" :options="deviceModeOptions" />
        <p class="settings-field__hint">
          {{ t("settings.deviceModeHint") }}
        </p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.allowLearning") }}</span>
        <button
          v-if="currentScene"
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': currentScene.learningEnabled }"
          :aria-pressed="currentScene.learningEnabled"
          @click="currentScene.learningEnabled = !currentScene.learningEnabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              currentScene.learningEnabled
                ? 'i-ri-checkbox-circle-fill'
                : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">
            {{
              currentScene?.learningEnabled
                ? t("settings.allowUpdateProfile")
                : t("settings.pauseUpdateProfile")
            }}
          </span>
        </button>
      </label>
    </div>

    <div class="settings-metrics-grid">
      <article v-for="item in networkMetricsCards" :key="item.label" class="settings-metric-card">
        <span class="settings-metric-card__label">{{ item.label }}</span>
        <strong class="settings-metric-card__value">{{ item.value }}</strong>
      </article>
    </div>
  </section>
</template>
