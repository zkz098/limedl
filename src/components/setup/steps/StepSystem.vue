<script setup lang="ts">
import { computed } from "vue";
import {
  enable as enableAutostart,
  disable as disableAutostart,
} from "@tauri-apps/plugin-autostart";
import { useI18n } from "../../../i18n";
import type { AppSettings, ProxyMode, ProxySettings } from "../../../types/settings";
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
import SettingsField from "../../settings/SettingsField.vue";
import UiSelect from "../../ui/UiSelect.vue";
import UiSwitch from "../../ui/UiSwitch.vue";
import UiTextField from "../../ui/UiTextField.vue";

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
  <StepShell
    icon="i-ri-settings-3-line"
    title-key="setupWizard.systemTitle"
    description-key="setupWizard.systemDescription"
  >
    <SettingsSection :title="t('settings.startupTitle')" icon="i-ri-shut-down-line">
      <SettingsField :hint="t('settings.autoStartHint')">
        <UiSwitch
          :model-value="settings.autostart"
          :label="t('settings.autoStart')"
          @update:model-value="onAutostartChange"
        />
      </SettingsField>
    </SettingsSection>

    <SettingsSection
      :title="t('settings.notificationSettings.title')"
      icon="i-ri-notification-3-line"
    >
      <SettingsField :hint="t('settings.notificationSettings.description')">
        <UiSwitch
          :model-value="settings.notifications.enabled"
          :label="t('settings.notificationSettings.toggleLabel')"
          @update:model-value="onNotificationChange"
        />
      </SettingsField>
    </SettingsSection>

    <SettingsSection :title="t('settings.proxyTitle')" icon="i-ri-global-line">
      <SettingsField :label="t('settings.proxyMode')">
        <UiSelect
          :model-value="settings.proxy.mode"
          :options="proxyModeOptions"
          @update:model-value="onProxyModeChange"
        />
      </SettingsField>

      <SettingsField
        v-if="settings.proxy.mode === 'manual'"
        :label="t('settings.proxyAddress')"
        :hint="t('settings.proxyHint')"
      >
        <UiTextField
          type="text"
          :model-value="settings.proxy.manualUrl"
          :placeholder="'http://127.0.0.1:7890'"
          @update:model-value="onProxyUrlChange"
        />
      </SettingsField>
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
/* Section-specific spacing is handled by SettingsField and SettingsSection. */
</style>
