<script setup lang="ts">
import { useI18n } from "../../../i18n";
import type { AppSettings, Aria2RpcSettings } from "../../../types/settings";
import UiSwitch from "../../ui/UiSwitch.vue";
import UiTextField from "../../ui/UiTextField.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

function updateRpc(patch: Partial<Aria2RpcSettings>) {
  emit("update:settings", {
    ...props.settings,
    aria2Rpc: { ...props.settings.aria2Rpc, ...patch },
  });
}

function onEnabledChange(enabled: boolean) {
  updateRpc({ enabled });
}

function onPortChange(port: string | number | null) {
  if (port === null || port === "") {
    updateRpc({ port: 6800 });
    return;
  }
  const value = typeof port === "number" ? port : Number.parseInt(port, 10);
  if (Number.isNaN(value)) {
    updateRpc({ port: 6800 });
    return;
  }
  updateRpc({ port: Math.max(1, Math.min(65535, value)) });
}

function onSecretChange(secret: string | number | null) {
  updateRpc({ secret: secret === "" || secret === null ? null : String(secret) });
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-server-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.rpcTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.rpcDescription") }}</p>
    <div class="setup-step__body">
      <div class="rpc-control">
        <UiSwitch
          :model-value="settings.aria2Rpc.enabled"
          :label="t('setupWizard.rpcEnableLabel')"
          @update:model-value="onEnabledChange"
        />
      </div>

      <div v-if="settings.aria2Rpc.enabled" class="rpc-fields">
        <div class="field-group">
          <label class="field-label">{{ t("setupWizard.rpcPortLabel") }}</label>
          <UiTextField
            type="number"
            :model-value="settings.aria2Rpc.port"
            :unit="t('setupWizard.rpcPortLabel')"
            :placeholder="'6800'"
            @update:model-value="onPortChange"
          />
          <p class="field-hint">{{ t("setupWizard.rpcPortHint") }}</p>
        </div>

        <div class="field-group">
          <label class="field-label">{{ t("setupWizard.rpcSecretLabel") }}</label>
          <UiTextField
            type="text"
            :model-value="settings.aria2Rpc.secret ?? ''"
            :placeholder="t('setupWizard.rpcSecretLabel')"
            @update:model-value="onSecretChange"
          />
          <p class="field-hint">{{ t("setupWizard.rpcSecretHint") }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup-step {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-6);
  flex: 1;
  min-height: 0;
  align-items: center;
  text-align: center;
}

.setup-step__header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
}

.setup-step__title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-heading);
}

.setup-step__description {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
  max-width: 480px;
}

.setup-step__body {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: center;
  gap: var(--space-4);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 560px;
}

.setup-step__icon {
  font-size: 2.5rem;
  color: var(--color-accent);
}

.rpc-control {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
}

.rpc-fields {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  text-align: left;
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.field-label {
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
}

.field-hint {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  line-height: var(--line-height-tight);
}
</style>
