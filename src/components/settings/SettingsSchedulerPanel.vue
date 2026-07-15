<script setup lang="ts">
import { computed } from "vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AdaptiveProfile, AppSettings, SchedulerMode } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

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
  <SettingsSection :title="t('settings.schedulerTitle')" icon="i-ri-git-branch-line">
    <div class="settings-grid">
      <SettingsField :label="t('settings.allocationMode')">
        <UiSelect v-model="draft.scheduler.mode" :options="schedulerModeOptions" />
      </SettingsField>

      <SettingsField v-if="isTraditional" :label="t('settings.maxParallelTasks')" :hint="t('settings.traditionalHint')">
        <UiTextField type="number" v-model="draft.scheduler.traditional.maxParallelTasks" :min="1" :max="32" />
      </SettingsField>

      <template v-else>
        <SettingsField :label="t('settings.maxParallelThreads')">
          <UiTextField
            type="number"
            v-model="draft.scheduler.automatic.maxParallelThreads"
            :min="1"
            :max="64"
          />
        </SettingsField>

        <SettingsField :label="t('settings.maxThreadsPerTask')">
          <UiTextField
            type="number"
            v-model="draft.scheduler.automatic.maxThreadsPerTask"
            :min="1"
            :max="maxThreadsPerTaskMax"
          />
        </SettingsField>

        <SettingsField :label="t('settings.minThreadsPerTask')" :hint="t('settings.minThreadsHint')">
          <UiTextField
            type="number"
            v-model="draft.scheduler.automatic.minThreadsPerTask"
            :min="0"
            :max="draft.scheduler.automatic.maxThreadsPerTask"
          />
        </SettingsField>

        <SettingsField wide :label="t('settings.adaptiveProfile')" :hint="t('settings.adaptiveProfileHint')">
          <UiSelect
            v-model="draft.scheduler.automatic.adaptiveProfile"
            :options="adaptiveProfileOptions"
          />
        </SettingsField>
      </template>

      <SettingsField wide :label="t('settings.globalSpeedLimit')" :hint="t('settings.globalSpeedLimitHint')">
        <UiTextField
          type="number"
          :model-value="globalSpeedLimitMiBps"
          :min="0"
          :max="1048576"
          unit="MiB/s"
          @update:model-value="emit('update:globalSpeedLimitMiBps', $event as number | null)"
        />
      </SettingsField>

      <SettingsField wide :label="t('settings.intelligentChunkAllocation')" :hint="t('settings.intelligentChunkAllocationHint')">
        <UiSwitch
          :model-value="draft.scheduler.chunkSizeStrategy === 'adaptive'"
          :label="t('settings.intelligentChunkAllocation')"
          @update:model-value="draft.scheduler.chunkSizeStrategy = $event ? 'adaptive' : 'fixed'"
        />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
