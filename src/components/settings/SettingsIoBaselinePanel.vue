<script setup lang="ts">
import { computed } from "vue";

import { formatBytes } from "../../lib/download-format";
import type { AppSettings } from "../../types/settings";
import UiCard from "../ui/UiCard.vue";
import UiUnitInput from "../ui/UiUnitInput.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  gameMode: boolean;
  bufferUsageBytes: number;
  bufferLimitBytes: number;
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

const bufferUsageText = computed(() => {
  const usage = formatBytes(props.bufferUsageBytes);
  const limit = formatBytes(props.bufferLimitBytes);
  return `${usage} / ${limit}`;
});
</script>

<template>
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <h3>{{ t("settings.ioBaseline.title") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-hard-drive-2-line" aria-hidden="true" />
      </div>
    </template>

    <div class="settings-grid">
      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.ioBaseline.bufferLimit") }}</span>
        <UiUnitInput v-model="bufferLimit" :min="64" :max="32768" unit="MB" />
        <p class="settings-field__hint">{{ t("settings.ioBaseline.bufferLimitHint") }}</p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.ioBaseline.gameModeBuffer") }}</span>
        <UiUnitInput
          v-model="gameModeBuffer"
          :min="16"
          :max="4096"
          :disabled="!gameMode"
          unit="MB"
        />
        <p class="settings-field__hint">{{ t("settings.ioBaseline.gameModeBufferHint") }}</p>
      </label>

      <div class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.ioBaseline.status") }}</span>
        <div class="io-status-bar">
          <span class="io-status-bar__label">{{ t("settings.ioBaseline.bufferUsage") }}</span>
          <span class="io-status-bar__value">{{ bufferUsageText }}</span>
        </div>
      </div>
    </div>
  </UiCard>
</template>

<style scoped>
.io-status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  min-height: 2.25rem;
  padding: 0 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  color: var(--color-text-main);
  font-size: 0.85rem;
}

.io-status-bar__label {
  color: var(--color-text-muted);
}

.io-status-bar__value {
  font-family: var(--font-mono);
  color: var(--color-heading);
}
</style>
