<script setup lang="ts">
import { computed } from "vue";
import UiInput from "../ui/UiInput.vue";
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
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.aria2Rpc") }}</p>
        <h3>{{ t("settings.aria2RpcTitle") }}</h3>
      </div>
      <span class="settings-section__icon i-ri-server-line" aria-hidden="true" />
    </div>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.aria2RpcService") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.aria2Rpc.enabled }"
          :aria-pressed="draft.aria2Rpc.enabled"
          @click="draft.aria2Rpc.enabled = !draft.aria2Rpc.enabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.aria2Rpc.enabled
                ? 'i-ri-checkbox-circle-fill'
                : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.aria2RpcService") }}</span>
        </button>
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
  </section>
</template>
