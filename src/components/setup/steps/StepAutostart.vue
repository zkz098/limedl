<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../../i18n";
import type { AppSettings, ProxyMode, ProxySettings } from "../../../types/settings";
import UiSelect from "../../ui/UiSelect.vue";
import UiSwitch from "../../ui/UiSwitch.vue";
import UiTextField from "../../ui/UiTextField.vue";
import {
  enable as enableAutostart,
  disable as disableAutostart,
} from "@tauri-apps/plugin-autostart";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

const proxyModeOptions = computed<{ label: string; value: ProxyMode }[]>(() => [
  { label: t("tokens.disabled"), value: "disabled" },
  { label: t("tokens.system"), value: "system" },
  { label: t("tokens.manual"), value: "manual" },
]);

function updateSettings(patch: Partial<AppSettings>) {
  emit("update:settings", { ...props.settings, ...patch });
}

function updateProxy(patch: Partial<ProxySettings>) {
  emit("update:settings", {
    ...props.settings,
    proxy: { ...props.settings.proxy, ...patch },
  });
}

async function onAutostartChange(value: boolean) {
  updateSettings({ autostart: value });
  try {
    if (value) {
      await enableAutostart();
    } else {
      await disableAutostart();
    }
  } catch {
    // ignore errors (e.g. permission denied in dev)
  }
}

function onNotificationChange(enabled: boolean) {
  updateSettings({ notifications: { ...props.settings.notifications, enabled } });
}

function onProxyModeChange(mode: ProxyMode) {
  updateProxy({ mode });
}

function onProxyUrlChange(value: string | number | null) {
  updateProxy({ manualUrl: value === null ? "" : String(value) });
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-settings-3-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.autostartTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.autostartDescription") }}</p>
    <div class="setup-step__body">
      <!-- Autostart -->
      <div class="system-control">
        <UiSwitch
          :model-value="settings.autostart"
          :label="t('settings.autoStart')"
          @update:model-value="onAutostartChange"
        />
        <p class="system-control__hint">{{ t("settings.autoStartHint") }}</p>
      </div>

      <!-- Notifications -->
      <div class="system-control">
        <UiSwitch
          :model-value="settings.notifications.enabled"
          :label="t('settings.notificationSettings.toggleLabel')"
          @update:model-value="onNotificationChange"
        />
      </div>

      <!-- Proxy -->
      <div class="system-control">
        <div class="field-group">
          <label class="field-label">{{ t("settings.proxyMode") }}</label>
          <UiSelect
            :model-value="settings.proxy.mode"
            :options="proxyModeOptions"
            @update:model-value="onProxyModeChange"
          />
        </div>

        <div v-if="settings.proxy.mode === 'manual'" class="field-group">
          <label class="field-label">{{ t("settings.proxyAddress") }}</label>
          <UiTextField
            type="text"
            :model-value="settings.proxy.manualUrl"
            :placeholder="'http://127.0.0.1:7890'"
            @update:model-value="onProxyUrlChange"
          />
          <p class="field-hint">{{ t("settings.proxyHint") }}</p>
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

.system-control {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel);
  text-align: left;
}

.system-control__hint {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  line-height: var(--line-height-tight);
  padding-left: var(--space-1);
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
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
