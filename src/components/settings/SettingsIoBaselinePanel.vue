<script setup lang="ts">
import { computed } from "vue";

import { formatBytes } from "../../lib/download-format";
import type { AppSettings } from "../../types/settings";
import UiTextField from "../ui/UiTextField.vue";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  gameMode: boolean;
  bufferUsageBytes: number;
  bufferLimitBytes: number;
  activeSlots: number;
  maxSlots: number;
  queuedCount: number;
}>();

const bufferLimit = computed({
  get: () => props.draft.ioBaseline.bufferLimitMb,
  set: (value: number | null) => {
    props.draft.ioBaseline.bufferLimitMb = Math.max(64, Math.min(32768, Math.trunc(value ?? 1024)));
  },
});

const gameModeBuffer = computed({
  get: () => props.draft.ioBaseline.gameModeBufferMb,
  set: (value: number | null) => {
    props.draft.ioBaseline.gameModeBufferMb = Math.max(
      16,
      Math.min(4096, Math.trunc(value ?? 128)),
    );
  },
});

const maxParallelHdd = computed({
  get: () => props.draft.ioBaseline.maxParallelHdd,
  set: (value: number | null) => {
    props.draft.ioBaseline.maxParallelHdd = Math.max(1, Math.min(16, Math.trunc(value ?? 4)));
  },
});

const gameModeMaxParallel = computed({
  get: () => props.draft.ioBaseline.gameModeMaxParallel,
  set: (value: number | null) => {
    props.draft.ioBaseline.gameModeMaxParallel = Math.max(1, Math.min(8, Math.trunc(value ?? 1)));
  },
});

const bufferUsageText = computed(() => {
  const usage = formatBytes(props.bufferUsageBytes);
  const limit = formatBytes(props.bufferLimitBytes);
  return `${usage} / ${limit}`;
});

const slotUsageText = computed(() => {
  return `${props.activeSlots} / ${props.maxSlots} (queued: ${props.queuedCount})`;
});
</script>

<template>
  <SettingsSection :title="t('settings.ioBaseline.title')" icon="i-ri-hard-drive-2-line">
    <div class="settings-grid">
      <SettingsField
        wide
        :label="t('settings.ioBaseline.bufferLimit')"
        :info-tooltip="t('settings.ioBaseline.bufferLimitHint')"
      >
        <UiTextField type="number" v-model="bufferLimit" :min="64" :max="32768" unit="MB" />
      </SettingsField>

      <SettingsField
        wide
        :label="t('settings.ioBaseline.gameModeBuffer')"
        :info-tooltip="t('settings.ioBaseline.gameModeBufferHint')"
      >
        <UiTextField
          type="number"
          v-model="gameModeBuffer"
          :min="16"
          :max="4096"
          :disabled="!gameMode"
          unit="MB"
        />
      </SettingsField>

      <SettingsField
        wide
        :label="t('settings.ioBaseline.maxParallelHdd')"
        :info-tooltip="t('settings.ioBaseline.maxParallelHddHint')"
      >
        <UiTextField type="number" v-model="maxParallelHdd" :min="1" :max="16" />
      </SettingsField>

      <SettingsField
        wide
        :label="t('settings.ioBaseline.gameModeMaxParallel')"
        :info-tooltip="t('settings.ioBaseline.gameModeMaxParallelHint')"
      >
        <UiTextField
          type="number"
          v-model="gameModeMaxParallel"
          :min="1"
          :max="8"
          :disabled="!gameMode"
        />
      </SettingsField>

      <SettingsField wide :label="t('settings.ioBaseline.status')">
        <div class="io-status-bar">
          <div class="io-status-bar__row">
            <span class="io-status-bar__label">{{ t("settings.ioBaseline.bufferUsage") }}</span>
            <span class="io-status-bar__value">{{ bufferUsageText }}</span>
          </div>
          <div class="io-status-bar__row">
            <span class="io-status-bar__label">{{ t("settings.ioBaseline.activeSlots") }}</span>
            <span class="io-status-bar__value">{{ slotUsageText }}</span>
          </div>
        </div>
      </SettingsField>
    </div>
  </SettingsSection>
</template>

<style scoped>
.io-status-bar {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-height: 2.25rem;
  padding: 0.5rem 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  color: var(--color-text-main);
  font-size: 0.85rem;
}

.io-status-bar__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.io-status-bar__label {
  color: var(--color-text-muted);
}

.io-status-bar__value {
  font-family: var(--font-mono);
  color: var(--color-heading);
}
</style>
