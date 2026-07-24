<script setup lang="ts">
import { computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
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

function openStore(url: string) {
  void openUrl(url);
}
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

      <SettingsField
        wide
        :label="t('settings.aria2RpcSecret')"
        :info-tooltip="t('settings.aria2RpcHint')"
      >
        <UiTextField v-model="secretModel" :placeholder="t('settings.aria2RpcSecretHint')" />
      </SettingsField>

      <SettingsField wide :label="t('settings.aria2RpcRecommendTitle')">
        <div class="aria2-recommend-card">
          <p class="aria2-recommend-card__desc">{{ t('settings.aria2RpcRecommendDesc') }}</p>
          <div class="aria2-recommend-card__stores">
            <button type="button" class="aria2-store-btn" @click="openStore('https://chromewebstore.google.com/detail/aria2-explorer/mpkodccbngfoacfalldjimigbofkhgjn')">
              <span class="i-ri-chrome-line" aria-hidden="true" />
              <span>Chrome</span>
            </button>
      <button type="button" class="aria2-store-btn" @click="openStore('https://chromewebstore.google.com/detail/aria2-explorer/mpkodccbngfoacfalldjimigbofkhgjn')">
        <span class="i-ri-edge-line" aria-hidden="true" />
        <span>Edge</span>
      </button>
            <button type="button" class="aria2-store-btn" @click="openStore('https://addons.mozilla.org/en-US/firefox/addon/ybbapp-aria2-explorer/')">
              <span class="i-ri-firefox-line" aria-hidden="true" />
              <span>Firefox</span>
            </button>
          </div>
          <p class="aria2-recommend-card__note">{{ t('settings.aria2RpcRecommendNote') }}</p>
        </div>
      </SettingsField>
    </div>
  </SettingsSection>
</template>

<style scoped>
.aria2-recommend-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-accent-soft-border);
  border-radius: var(--radius-md);
  background: var(--color-accent-soft);
}

.aria2-recommend-card__desc {
  margin: 0;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  color: var(--color-text-main);
}

.aria2-recommend-card__stores {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.aria2-store-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-accent-border);
  border-radius: var(--radius-pill);
  background: var(--color-panel);
  color: var(--color-accent-strong);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  transition: background-color 0.2s ease, transform 0.15s ease;
}

.aria2-store-btn:hover {
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  transform: translateY(-1px);
}

.aria2-store-btn:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.aria2-recommend-card__note {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--color-text-muted);
}
</style>
