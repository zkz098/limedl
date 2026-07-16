<script setup lang="ts">
import { computed } from "vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
}>();

const portModel = computed({
  get: () => String(props.draft.aria2Rpc.port),
  set: (value: string) => {
    const parsed = parseInt(value, 10);
    if (!Number.isNaN(parsed) && parsed >= 1 && parsed <= 65535) {
      props.draft.aria2Rpc.port = parsed;
    } else if (value === "") {
      props.draft.aria2Rpc.port = 6800;
    }
  },
});

const secretModel = computed({
  get: () => props.draft.aria2Rpc.secret ?? "",
  set: (value: string) => {
    props.draft.aria2Rpc.secret = value.trim() || null;
  },
});
</script>

<template>
  <SettingsSection :title="t('settings.aria2RpcTitle')" icon="i-ri-server-line">
    <div class="settings-grid">
      <SettingsField :label="t('settings.aria2RpcService')">
        <UiSwitch v-model="draft.aria2Rpc.enabled" :label="t('settings.aria2RpcService')" />
      </SettingsField>

      <SettingsField :label="t('settings.aria2RpcPort')">
        <UiTextField v-model="portModel" placeholder="6800" />
      </SettingsField>

      <SettingsField wide :label="t('settings.aria2RpcSecret')" :info-tooltip="t('settings.aria2RpcHint')">
        <UiTextField v-model="secretModel" :placeholder="t('settings.aria2RpcSecretHint')" />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
