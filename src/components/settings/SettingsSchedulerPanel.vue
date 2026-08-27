<script setup lang="ts">
import { computed, ref, useId } from "vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import UiButton from "../ui/UiButton.vue";
import type { AdaptiveProfile, AppSettings, SchedulerMode } from "../../types/settings";
import type { SpeedLimitSlot } from "../../types/generated/types";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const emit = defineEmits<{
  "update:globalSpeedLimitMiBps": [value: number | null];
}>();

const draft = defineModel<AppSettings>("draft", { required: true });
const uid = useId();

const props = defineProps<{
  t: (key: string, options?: Record<string, unknown>) => string;
  schedulerModeOptions: Array<{ label: string; value: SchedulerMode }>;
  adaptiveProfileOptions: Array<{ label: string; value: AdaptiveProfile }>;
  globalSpeedLimitMiBps: number;
}>();

type ViewMode = "simple" | "custom";
type PresetKey = "energySaver" | "balanced" | "maxSpeed";
type PerformancePreset = PresetKey | "custom";

interface PresetConfig {
  mode: SchedulerMode;
  maxParallelThreads: number;
  maxThreadsPerTask: number;
  minThreadsPerTask: number;
  adaptiveProfile: AdaptiveProfile;
}

// PRESETS are ordered by priority for ambiguous matching.
// If multiple presets share the same config values, the first match wins.
// Keep keys in priority order: energySaver → balanced → maxSpeed.
const PRESETS: Record<PresetKey, PresetConfig> = {
  energySaver: {
    mode: "automatic",
    maxParallelThreads: 8,
    maxThreadsPerTask: 4,
    minThreadsPerTask: 2,
    adaptiveProfile: "conservative",
  },
  balanced: {
    mode: "automatic",
    maxParallelThreads: 16,
    maxThreadsPerTask: 8,
    minThreadsPerTask: 2,
    adaptiveProfile: "balanced",
  },
  maxSpeed: {
    mode: "automatic",
    maxParallelThreads: 32,
    maxThreadsPerTask: 16,
    minThreadsPerTask: 4,
    adaptiveProfile: "aggressive",
  },
};

const viewMode = ref<ViewMode>("simple");

const performancePreset = computed<PerformancePreset>(() => {
  const { mode, automatic } = draft.value.scheduler;
  if (mode === "traditional") return "custom";

  // Iteration order matches PRESETS declaration order (ES2015+ stable insertion order).
  // First matching preset wins — see PRESETS comment above.
  for (const [key, config] of Object.entries(PRESETS) as Array<[PresetKey, PresetConfig]>) {
    if (
      automatic.maxParallelThreads === config.maxParallelThreads &&
      automatic.maxThreadsPerTask === config.maxThreadsPerTask &&
      automatic.minThreadsPerTask === config.minThreadsPerTask &&
      automatic.adaptiveProfile === config.adaptiveProfile
    ) {
      return key;
    }
  }
  return "custom";
});

const presetCards = computed<
  Array<{ value: PresetKey; icon: string; title: string; description: string }>
>(() => [
  {
    value: "energySaver",
    icon: "i-ri-leaf-line",
    title: props.t("settings.performancePresetEnergySaver"),
    description: props.t("settings.performancePresetEnergySaverHint"),
  },
  {
    value: "balanced",
    icon: "i-ri-scales-3-line",
    title: props.t("settings.performancePresetBalanced"),
    description: props.t("settings.performancePresetBalancedHint"),
  },
  {
    value: "maxSpeed",
    icon: "i-ri-rocket-line",
    title: props.t("settings.performancePresetMaxSpeed"),
    description: props.t("settings.performancePresetMaxSpeedHint"),
  },
]);

function applyPreset(preset: PresetKey) {
  const config = PRESETS[preset];
  draft.value.scheduler.mode = config.mode;
  draft.value.scheduler.automatic.maxParallelThreads = config.maxParallelThreads;
  draft.value.scheduler.automatic.maxThreadsPerTask = config.maxThreadsPerTask;
  draft.value.scheduler.automatic.minThreadsPerTask = config.minThreadsPerTask;
  draft.value.scheduler.automatic.adaptiveProfile = config.adaptiveProfile;
}

const maxThreadsPerTaskMax = computed(() =>
  Math.max(1, draft.value.scheduler.automatic.maxParallelThreads),
);

const isTraditional = computed(() => draft.value.scheduler.mode === "traditional");

const hourOptions = computed(() =>
  Array.from({ length: 24 }, (_, i) => ({ label: String(i), value: i })),
);

const scheduleEnabled = computed({
  get: () => draft.value.speedLimitSchedule.length > 0,
  set: (enabled: boolean) => {
    if (enabled) {
      draft.value.speedLimitSchedule = draft.value.speedLimitSchedule.length
        ? draft.value.speedLimitSchedule
        : [{ startHour: 0, endHour: 6, limitBps: 0 }];
    } else {
      draft.value.speedLimitSchedule = [];
    }
  },
});

function slotLimitMiBps(slot: SpeedLimitSlot): number {
  return Math.round(slot.limitBps / 1024 / 1024);
}

function setSlotLimit(slot: SpeedLimitSlot, value: number | null) {
  slot.limitBps = Math.max(0, Math.trunc(value ?? 0)) * 1024 * 1024;
}

function removeSlot(index: number) {
  draft.value.speedLimitSchedule.splice(index, 1);
}

function addSlot() {
  draft.value.speedLimitSchedule.push({ startHour: 0, endHour: 6, limitBps: 0 });
}

function slotWrapsMidnight(slot: SpeedLimitSlot) {
  return slot.startHour >= slot.endHour;
}
</script>

<template>
  <SettingsSection :title="t('settings.schedulerTitle')" icon="i-ri-git-branch-line">
    <div class="scheduler-panel__header">
      <div class="view-toggle" role="tablist" :aria-label="t('settings.schedulerViewMode')">
        <button
          type="button"
          class="view-toggle__button"
          :class="{ 'is-active': viewMode === 'simple' }"
          role="tab"
          :aria-selected="viewMode === 'simple'"
          @click="viewMode = 'simple'"
        >
          <span class="i-ri-dashboard-line" aria-hidden="true" />
          {{ t("settings.simpleView") }}
        </button>
        <button
          type="button"
          class="view-toggle__button"
          :class="{ 'is-active': viewMode === 'custom' }"
          role="tab"
          :aria-selected="viewMode === 'custom'"
          @click="viewMode = 'custom'"
        >
          <span class="i-ri-sliders-line" aria-hidden="true" />
          {{ t("settings.customView") }}
        </button>
      </div>
    </div>

    <div class="settings-grid">
      <SettingsField
        wide
        no-association
        :label="t('settings.performancePreference')"
        :info-tooltip="t('settings.performancePreferenceHint')"
      >
        <fieldset class="performance-presets" :aria-label="t('settings.performancePreference')">
          <label
            v-for="card in presetCards"
            :key="card.value"
            class="performance-preset-card"
            :class="{ 'is-active': performancePreset === card.value }"
          >
            <input
              type="radio"
              name="performancePreset"
              :value="card.value"
              :checked="performancePreset === card.value"
              @change="applyPreset(card.value)"
            />
            <span :class="card.icon" class="performance-preset-card__icon" aria-hidden="true" />
            <span class="performance-preset-card__title">{{ card.title }}</span>
            <span class="performance-preset-card__desc">{{ card.description }}</span>
          </label>
        </fieldset>
      </SettingsField>
    </div>

    <Transition name="scheduler-fade" mode="out-in">
      <div v-if="viewMode === 'simple'" key="simple" class="settings-grid">
        <SettingsField
          wide
          :label="t('settings.globalSpeedLimit')"
          :info-tooltip="t('settings.globalSpeedLimitHint')"
        >
          <UiTextField
            type="number"
            :model-value="globalSpeedLimitMiBps"
            :min="0"
            :max="1048576"
            unit="MiB/s"
            @update:model-value="emit('update:globalSpeedLimitMiBps', $event as number | null)"
          />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.intelligentChunking')"
          :info-tooltip="t('settings.intelligentChunkingHint')"
        >
          <UiSwitch
            :model-value="draft.scheduler.chunkSizeStrategy === 'adaptive'"
            :label="t('settings.intelligentChunking')"
            @update:model-value="draft.scheduler.chunkSizeStrategy = $event ? 'adaptive' : 'fixed'"
          />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.tailSprint')"
          :info-tooltip="t('settings.tailSprintHint')"
        >
          <UiSwitch v-model="draft.scheduler.tailSprintEnabled" :label="t('settings.tailSprint')" />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.connectionWarmup')"
          :info-tooltip="t('settings.connectionWarmupHint')"
        >
          <UiSwitch
            v-model="draft.scheduler.connectionWarmupEnabled"
            :label="t('settings.connectionWarmup')"
          />
        </SettingsField>
      </div>

      <div v-else key="custom" class="settings-grid">
        <SettingsField :label="t('settings.allocationMode')">
          <UiSelect v-model="draft.scheduler.mode" :options="schedulerModeOptions" />
        </SettingsField>

        <SettingsField
          v-if="isTraditional"
          :label="t('settings.maxParallelTasks')"
          :hint="t('settings.traditionalHint')"
        >
          <UiTextField
            type="number"
            v-model="draft.scheduler.traditional.maxParallelTasks"
            :min="1"
            :max="32"
          />
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

          <SettingsField
            :label="t('settings.minThreadsPerTask')"
            :hint="t('settings.minThreadsHint')"
          >
            <UiTextField
              type="number"
              v-model="draft.scheduler.automatic.minThreadsPerTask"
              :min="0"
              :max="draft.scheduler.automatic.maxThreadsPerTask"
            />
          </SettingsField>

          <SettingsField
            wide
            :label="t('settings.adaptiveProfile')"
            :info-tooltip="t('settings.adaptiveProfileHint')"
          >
            <UiSelect
              v-model="draft.scheduler.automatic.adaptiveProfile"
              :options="adaptiveProfileOptions"
            />
          </SettingsField>
        </template>

        <SettingsField
          wide
          :label="t('settings.globalSpeedLimit')"
          :info-tooltip="t('settings.globalSpeedLimitHint')"
        >
          <UiTextField
            type="number"
            :model-value="globalSpeedLimitMiBps"
            :min="0"
            :max="1048576"
            unit="MiB/s"
            @update:model-value="emit('update:globalSpeedLimitMiBps', $event as number | null)"
          />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.intelligentChunkAllocation')"
          :info-tooltip="t('settings.intelligentChunkAllocationHint')"
        >
          <UiSwitch
            :model-value="draft.scheduler.chunkSizeStrategy === 'adaptive'"
            :label="t('settings.intelligentChunkAllocation')"
            @update:model-value="draft.scheduler.chunkSizeStrategy = $event ? 'adaptive' : 'fixed'"
          />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.tailSprint')"
          :info-tooltip="t('settings.tailSprintHint')"
        >
          <UiSwitch v-model="draft.scheduler.tailSprintEnabled" :label="t('settings.tailSprint')" />
        </SettingsField>

        <SettingsField
          wide
          :label="t('settings.connectionWarmup')"
          :info-tooltip="t('settings.connectionWarmupHint')"
        >
          <UiSwitch
            v-model="draft.scheduler.connectionWarmupEnabled"
            :label="t('settings.connectionWarmup')"
          />
        </SettingsField>
      </div>
    </Transition>

    <div class="settings-grid">
      <SettingsField
        wide
        no-association
        :label="t('settings.speedLimitSchedule')"
        :info-tooltip="t('settings.speedLimitScheduleHint')"
      >
        <UiSwitch v-model="scheduleEnabled" :label="t('settings.speedLimitScheduleEnable')" />

        <Transition name="schedule-fade">
          <div v-if="scheduleEnabled" class="speed-limit-schedule">
            <ul v-if="draft.speedLimitSchedule.length" class="speed-limit-schedule__list">
              <li
                v-for="(slot, index) in draft.speedLimitSchedule"
                :key="index"
                class="speed-limit-schedule__slot"
              >
                <div class="speed-limit-schedule__field">
                  <label class="speed-limit-schedule__field-label" :for="`${uid}-start-${index}`">
                    {{ t("settings.speedLimitScheduleStartHour") }}
                  </label>
                  <UiSelect
                    v-model="slot.startHour"
                    :id="`${uid}-start-${index}`"
                    :options="hourOptions"
                  />
                </div>
                <div class="speed-limit-schedule__field">
                  <label class="speed-limit-schedule__field-label" :for="`${uid}-end-${index}`">
                    {{ t("settings.speedLimitScheduleEndHour") }}
                  </label>
                  <UiSelect
                    v-model="slot.endHour"
                    :id="`${uid}-end-${index}`"
                    :options="hourOptions"
                  />
                </div>
                <div class="speed-limit-schedule__field speed-limit-schedule__field--limit">
                  <label class="speed-limit-schedule__field-label" :for="`${uid}-limit-${index}`">
                    {{ t("settings.speedLimitScheduleLimit") }}
                  </label>
                  <UiTextField
                    type="number"
                    :id="`${uid}-limit-${index}`"
                    :model-value="slotLimitMiBps(slot)"
                    :min="0"
                    :max="1048576"
                    unit="MiB/s"
                    @update:model-value="setSlotLimit(slot, $event as number | null)"
                  />
                </div>
                <span v-if="slotWrapsMidnight(slot)" class="speed-limit-schedule__wraps text-xs">
                  {{ t("settings.speedLimitScheduleWrapsMidnight") }}
                </span>
                <UiButton
                  size="sm"
                  variant="ghost"
                  icon="i-ri-delete-bin-line"
                  class="speed-limit-schedule__remove"
                  :title="t('settings.speedLimitScheduleRemove')"
                  @click="removeSlot(index)"
                />
              </li>
            </ul>
            <p v-else class="speed-limit-schedule__empty">
              {{ t("settings.speedLimitScheduleEmpty") }}
            </p>
            <UiButton
              variant="secondary"
              size="sm"
              icon="i-ri-add-line"
              block
              class="speed-limit-schedule__add"
              @click="addSlot"
            >
              {{ t("settings.speedLimitScheduleAdd") }}
            </UiButton>
            <p class="speed-limit-schedule__hint">
              {{ t("settings.speedLimitScheduleSpeedHint") }}
            </p>
          </div>
        </Transition>
      </SettingsField>
    </div>
  </SettingsSection>
</template>

<style scoped>
.scheduler-panel__header {
  display: flex;
  justify-content: flex-end;
  margin-bottom: var(--space-4);
}

.view-toggle {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
}

.view-toggle__button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  transition:
    color 0.2s ease,
    background-color 0.2s ease;
}

.view-toggle__button:hover {
  color: var(--color-text-main);
}

.view-toggle__button.is-active {
  background: var(--color-panel);
  color: var(--color-accent-strong);
  box-shadow: var(--shadow-soft);
}

.view-toggle__button:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.performance-presets {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  border: 0;
  min-inline-size: 0;
}

.performance-preset-card input[type="radio"] {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0 0 0 0);
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}

.performance-preset-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
  color: var(--color-text-main);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease,
    box-shadow 0.2s ease;
}

.performance-preset-card:hover {
  border-color: var(--color-border-strong);
  background: var(--color-surface-hover);
}

.performance-preset-card.is-active {
  border-color: var(--color-accent-border);
  background: var(--color-accent-soft);
  box-shadow: var(--shadow-accent);
}

.performance-preset-card:focus-visible,
.performance-preset-card:has(input[type="radio"]:focus-visible) {
  outline: none;
  border-color: var(--color-accent-strong);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.performance-preset-card__icon {
  font-size: var(--font-size-metric);
  color: var(--color-accent);
}

.performance-preset-card.is-active .performance-preset-card__icon {
  color: var(--color-accent-strong);
}

.performance-preset-card__title {
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
  line-height: 1.2;
}

.performance-preset-card__desc {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  line-height: 1.5;
}

.scheduler-fade-enter-active,
.scheduler-fade-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.scheduler-fade-enter-from,
.scheduler-fade-leave-to {
  opacity: 0;
  transform: translateY(4px);
}

.schedule-fade-enter-active,
.schedule-fade-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.schedule-fade-enter-from,
.schedule-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.speed-limit-schedule {
  margin-top: var(--space-3);
  display: grid;
  gap: var(--space-2);
}

.speed-limit-schedule__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: var(--space-2);
}

.speed-limit-schedule__slot {
  display: flex;
  gap: var(--space-3);
  align-items: center;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  flex-wrap: wrap;
}

.speed-limit-schedule__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 4rem;
  flex: 1 1 auto;
}

.speed-limit-schedule__field--limit {
  flex: 2 1 auto;
  min-width: 7rem;
}

.speed-limit-schedule__field-label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.speed-limit-schedule__wraps {
  color: var(--color-text-muted);
  white-space: nowrap;
}

.speed-limit-schedule__remove {
  color: var(--color-text-muted);
  flex: none;
}

.speed-limit-schedule__remove:hover:not(:disabled) {
  color: var(--color-danger-text);
  background: var(--color-danger-bg);
}

.speed-limit-schedule__empty {
  margin: 0;
  padding: var(--space-3);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  text-align: center;
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
}

.speed-limit-schedule__add {
  margin-top: var(--space-1);
}

.speed-limit-schedule__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

@media (max-width: 840px) {
  .performance-presets {
    grid-template-columns: 1fr;
  }
}
</style>
