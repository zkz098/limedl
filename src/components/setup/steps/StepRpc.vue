<script setup lang="ts">
import { useI18n } from "../../../i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppSettings, Aria2RpcSettings } from "../../../types/settings";
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
import SettingsField from "../../settings/SettingsField.vue";
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

function onRpcEnabledChange(enabled: boolean) {
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

function openStore(url: string) {
  void openUrl(url);
}
</script>

<template>
  <StepShell
    icon="i-ri-server-line"
    title-key="setupWizard.rpcTitle"
    description-key="setupWizard.rpcDescription"
  >
    <SettingsSection
      :title="t('setupWizard.rpcTitle')"
      icon="i-ri-server-line"
      :summary="t('setupWizard.rpcDescription')"
    >
      <SettingsField>
        <UiSwitch
          :model-value="settings.aria2Rpc.enabled"
          :label="t('setupWizard.rpcEnableLabel')"
          @update:model-value="onRpcEnabledChange"
        />
      </SettingsField>

      <div v-if="settings.aria2Rpc.enabled" class="rpc-fields">
        <SettingsField :label="t('setupWizard.rpcPortLabel')" :hint="t('setupWizard.rpcPortHint')">
          <UiTextField
            type="number"
            :model-value="settings.aria2Rpc.port"
            :placeholder="'6800'"
            @update:model-value="onPortChange"
          />
        </SettingsField>

        <SettingsField
          :label="t('setupWizard.rpcSecretLabel')"
          :hint="t('setupWizard.rpcSecretHint')"
        >
          <UiTextField
            type="text"
            :model-value="settings.aria2Rpc.secret ?? ''"
            :placeholder="t('setupWizard.rpcSecretLabel')"
            @update:model-value="onSecretChange"
          />
        </SettingsField>
      </div>

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
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
.rpc-fields {
  display: grid;
  grid-template-columns: 1fr;
  gap: var(--space-4);
  text-align: left;
}

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
