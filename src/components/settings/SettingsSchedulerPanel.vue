<script setup lang="ts">
import { computed } from "vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { AdaptiveProfile, AppSettings, SchedulerMode } from "../../types/settings";

const emit = defineEmits<{
  "update:globalSpeedLimitMiBps": [value: number | null];
}>();

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  schedulerModeOptions: Array<{ label: string; value: SchedulerMode }>;
  adaptiveProfileOptions: Array<{ label: string; value: AdaptiveProfile }>;
  globalSpeedLimitMiBps: number;
}>();

const maxThreadsPerTaskMax = computed(() =>
  Math.max(1, props.draft.scheduler.automatic.maxParallelThreads),
);

const isTraditional = computed(() => props.draft.scheduler.mode === "traditional");
</script>

<template>
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.scheduler") }}</p>
        <h3>{{ t("settings.schedulerTitle") }}</h3>
      </div>
      <span class="settings-section__icon i-ri-git-branch-line" aria-hidden="true" />
    </div>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.allocationMode") }}</span>
        <UiSelect v-model="draft.scheduler.mode" :options="schedulerModeOptions" />
      </label>

      <label v-if="isTraditional" class="settings-field">
        <span class="settings-field__label">{{ t("settings.maxParallelTasks") }}</span>
        <UiNumberField v-model="draft.scheduler.traditional.maxParallelTasks" :min="1" :max="32" />
        <p class="settings-field__hint">{{ t("settings.traditionalHint") }}</p>
      </label>

      <template v-else>
        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.maxParallelThreads") }}</span>
          <UiNumberField
            v-model="draft.scheduler.automatic.maxParallelThreads"
            :min="1"
            :max="64"
          />
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.maxThreadsPerTask") }}</span>
          <UiNumberField
            v-model="draft.scheduler.automatic.maxThreadsPerTask"
            :min="1"
            :max="maxThreadsPerTaskMax"
          />
        </label>

        <label class="settings-field">
          <span class="settings-field__label">{{ t("settings.minThreadsPerTask") }}</span>
          <UiNumberField
            v-model="draft.scheduler.automatic.minThreadsPerTask"
            :min="0"
            :max="draft.scheduler.automatic.maxThreadsPerTask"
          />
          <p class="settings-field__hint">{{ t("settings.minThreadsHint") }}</p>
        </label>

        <label class="settings-field settings-field--wide">
          <span class="settings-field__label">{{ t("settings.adaptiveProfile") }}</span>
          <UiSelect
            v-model="draft.scheduler.automatic.adaptiveProfile"
            :options="adaptiveProfileOptions"
          />
          <p class="settings-field__hint">
            {{ t("settings.adaptiveProfileHint") }}
          </p>
        </label>
      </template>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.globalSpeedLimit") }}</span>
        <UiNumberField
          :model-value="globalSpeedLimitMiBps"
          :min="0"
          :max="1048576"
          @update:model-value="emit('update:globalSpeedLimitMiBps', $event)"
        />
        <p class="settings-field__hint">{{ t("settings.globalSpeedLimitHint") }}</p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.intelligentChunkAllocation") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.scheduler.chunkSizeStrategy === 'adaptive' }"
          :aria-pressed="draft.scheduler.chunkSizeStrategy === 'adaptive'"
          @click="draft.scheduler.chunkSizeStrategy = draft.scheduler.chunkSizeStrategy === 'adaptive' ? 'fixed' : 'adaptive'"
        >
          <span
            class="settings-toggle__icon"
            :class="draft.scheduler.chunkSizeStrategy === 'adaptive' ? 'i-ri-checkbox-circle-fill' : 'i-ri-checkbox-blank-circle-line'"
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.intelligentChunkAllocation") }}</span>
        </button>
        <p class="settings-field__hint">{{ t("settings.intelligentChunkAllocationHint") }}</p>
      </label>
    </div>
  </section>
</template>
