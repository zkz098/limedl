<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../../i18n";
import type { AppSettings, AdaptiveProfile, SchedulerMode, SchedulerSettings } from "../../../types/settings";
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
import SettingsField from "../../settings/SettingsField.vue";
import UiSwitch from "../../ui/UiSwitch.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

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

const performancePreset = computed<PerformancePreset>(() => {
  const { mode, automatic } = props.settings.scheduler;
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
    title: t("settings.performancePresetEnergySaver"),
    description: t("settings.performancePresetEnergySaverHint"),
  },
  {
    value: "balanced",
    icon: "i-ri-scales-3-line",
    title: t("settings.performancePresetBalanced"),
    description: t("settings.performancePresetBalancedHint"),
  },
  {
    value: "maxSpeed",
    icon: "i-ri-rocket-line",
    title: t("settings.performancePresetMaxSpeed"),
    description: t("settings.performancePresetMaxSpeedHint"),
  },
]);

function updateScheduler(patch: Partial<SchedulerSettings>) {
  emit("update:settings", {
    ...props.settings,
    scheduler: { ...props.settings.scheduler, ...patch },
  });
}

function applyPreset(preset: PresetKey) {
  const config = PRESETS[preset];
  updateScheduler({
    mode: config.mode,
    automatic: {
      ...props.settings.scheduler.automatic,
      maxParallelThreads: config.maxParallelThreads,
      maxThreadsPerTask: config.maxThreadsPerTask,
      minThreadsPerTask: config.minThreadsPerTask,
      adaptiveProfile: config.adaptiveProfile,
    },
  });
}

function onChunkStrategyChange(enabled: boolean) {
  updateScheduler({ chunkSizeStrategy: enabled ? "adaptive" : "fixed" });
}
</script>

<template>
  <StepShell
    icon="i-ri-speed-up-line"
    title-key="setupWizard.performanceTitle"
    description-key="setupWizard.performanceDescription"
  >
    <SettingsSection
      :title="t('settings.performancePreference')"
      icon="i-ri-dashboard-line"
      :summary="t('settings.performancePreferenceHint')"
    >
      <div
        class="performance-presets"
        role="radiogroup"
        :aria-label="t('settings.performancePreference')"
      >
        <button
          v-for="card in presetCards"
          :key="card.value"
          type="button"
          class="performance-preset-card"
          :class="{ 'is-active': performancePreset === card.value }"
          role="radio"
          :aria-checked="performancePreset === card.value"
          @click="applyPreset(card.value)"
        >
          <span :class="card.icon" class="performance-preset-card__icon" aria-hidden="true" />
          <span class="performance-preset-card__title">{{ card.title }}</span>
          <span class="performance-preset-card__desc">{{ card.description }}</span>
        </button>
      </div>
    </SettingsSection>

    <SettingsSection
      :title="t('settings.intelligentChunking')"
      icon="i-ri-grid-line"
      :summary="t('settings.intelligentChunkingHint')"
    >
      <SettingsField>
        <UiSwitch
          :model-value="settings.scheduler.chunkSizeStrategy === 'adaptive'"
          :label="t('settings.intelligentChunking')"
          @update:model-value="onChunkStrategyChange"
        />
      </SettingsField>
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
.performance-presets {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
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

.performance-preset-card:focus-visible {
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

@media (max-width: 840px) {
  .performance-presets {
    grid-template-columns: 1fr;
  }
}
</style>
