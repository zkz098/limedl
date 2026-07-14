<script setup lang="ts">
import { computed } from "vue";
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings } from "../../types/settings";

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
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">{{ t("settings.aria2Rpc") }}</p>
          <h3>{{ t("settings.aria2RpcTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-server-line" aria-hidden="true" />
      </div>
    </template>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.aria2RpcService") }}</span>
        <UiSwitch v-model="draft.aria2Rpc.enabled" :label="t('settings.aria2RpcService')" />
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.aria2RpcPort") }}</span>
        <UiInput v-model="portModel" placeholder="6800" />
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.aria2RpcSecret") }}</span>
        <UiInput v-model="secretModel" :placeholder="t('settings.aria2RpcSecretHint')" />
        <p class="settings-field__hint">{{ t("settings.aria2RpcHint") }}</p>
      </label>
    </div>
  </UiCard>
</template>
