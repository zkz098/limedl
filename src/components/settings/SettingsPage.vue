<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";

import { getProxySettings, saveProxySettings } from "../../lib/tauri/settings-api";
import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiSelect from "../ui/UiSelect.vue";
import type { ProxyMode, ProxySettings } from "../../types/settings";

const proxyModeOptions: Array<{ label: string; value: ProxyMode }> = [
  { label: "不使用代理", value: "disabled" },
  { label: "系统代理", value: "system" },
  { label: "手动设置代理", value: "manual" },
];

const proxySettings = reactive<ProxySettings>({
  mode: "disabled",
  manualUrl: "",
});
const isLoading = ref(true);
const isSaving = ref(false);
const notificationMessage = ref("");
let notificationTimer: ReturnType<typeof setTimeout> | null = null;

const proxySummary = computed(() => {
  if (proxySettings.mode === "disabled") {
    return "当前直接连接，不经过代理。";
  }

  if (proxySettings.mode === "system") {
    return "当前将跟随系统代理配置。";
  }

  return proxySettings.manualUrl.trim()
    ? `当前手动代理：${proxySettings.manualUrl.trim()}`
    : "请输入代理地址，例如 http://127.0.0.1:7890";
});

function showNotification(message: string) {
  notificationMessage.value = message;
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
  notificationTimer = setTimeout(() => {
    notificationMessage.value = "";
    notificationTimer = null;
  }, 2200);
}

async function loadSettings() {
  try {
    const settings = await getProxySettings();
    proxySettings.mode = settings.mode;
    proxySettings.manualUrl = settings.manualUrl;
  } catch (error) {
    showNotification(error instanceof Error ? error.message : "读取代理设置失败");
  } finally {
    isLoading.value = false;
  }
}

async function persistSettings() {
  if (isSaving.value) {
    return;
  }

  isSaving.value = true;

  try {
    const saved = await saveProxySettings({
      mode: proxySettings.mode,
      manualUrl: proxySettings.manualUrl,
    });
    proxySettings.mode = saved.mode;
    proxySettings.manualUrl = saved.manualUrl;
    showNotification("代理设置已保存");
  } catch (error) {
    showNotification(error instanceof Error ? error.message : "保存代理设置失败");
  } finally {
    isSaving.value = false;
  }
}

onMounted(() => {
  void loadSettings();
});

onBeforeUnmount(() => {
  if (notificationTimer) {
    clearTimeout(notificationTimer);
  }
});
</script>

<template>
  <section class="settings-page">
    <Transition name="settings-notification">
      <div v-if="notificationMessage" class="settings-notification" role="status">
        <span class="i-ri-checkbox-circle-line" aria-hidden="true" />
        <span>{{ notificationMessage }}</span>
      </div>
    </Transition>

    <div class="desk-panel__header settings-page__header">
      <div>
        <p class="section-kicker">Settings</p>
        <h2 class="panel-title">设置</h2>
      </div>
      <p class="settings-page__summary">{{ proxySummary }}</p>
    </div>

    <section class="settings-section">
      <div class="settings-section__head">
        <div>
          <p class="section-kicker">Network</p>
          <h3>代理</h3>
        </div>
        <span class="settings-section__icon i-ri-global-line" aria-hidden="true" />
      </div>

      <div class="settings-grid">
        <label class="settings-field">
          <span class="settings-field__label">代理模式</span>
          <UiSelect
            v-model="proxySettings.mode"
            :options="proxyModeOptions"
            :disabled="isLoading"
          />
        </label>

        <label v-if="proxySettings.mode === 'manual'" class="settings-field settings-field--wide">
          <span class="settings-field__label">代理地址</span>
          <UiInput
            v-model="proxySettings.manualUrl"
            type="text"
            placeholder="http://127.0.0.1:7890"
            :disabled="isLoading"
          />
          <p class="settings-field__hint">
            支持常见 HTTP / HTTPS / SOCKS 代理地址，按完整 URL 填写。
          </p>
        </label>
      </div>

      <div class="settings-actions">
        <UiButton
          type="button"
          variant="secondary"
          icon="i-ri-save-line"
          :disabled="isLoading || isSaving"
          @click="persistSettings"
        >
          {{ isSaving ? "保存中…" : "保存设置" }}
        </UiButton>
      </div>
    </section>
  </section>
</template>

<style scoped>
.settings-page {
  display: grid;
  gap: 1rem;
}

.settings-notification {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 40;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 0.9rem;
  border: 1px solid var(--color-success-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) 96%, transparent);
  box-shadow: var(--shadow-card-hover);
  color: var(--color-success-text);
  font-size: 0.85rem;
  backdrop-filter: blur(0.875rem);
}

.settings-page__header {
  align-items: flex-end;
}

.settings-page__summary {
  margin: 0;
  max-width: 28rem;
  color: var(--color-text-muted);
  font-size: 0.88rem;
  line-height: 1.55;
  text-align: right;
}

.settings-section {
  display: grid;
  gap: 1rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  background: color-mix(in srgb, var(--color-panel) 94%, transparent);
  box-shadow: var(--shadow-card);
}

.settings-section__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.settings-section__head h3 {
  margin: 0.2rem 0 0;
  color: var(--color-heading);
  font-size: 1rem;
}

.settings-section__icon {
  width: 2.25rem;
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 10%, var(--color-panel-muted));
  border: 1px solid color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
}

.settings-field {
  display: grid;
  gap: 0.45rem;
  min-width: 0;
}

.settings-field--wide {
  grid-column: 1 / -1;
}

.settings-field__label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.settings-field__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.82rem;
  line-height: 1.5;
}

.settings-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 1rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--color-border);
}

.settings-notification-enter-active,
.settings-notification-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.settings-notification-enter-from,
.settings-notification-leave-to {
  opacity: 0;
  transform: translateY(-0.45rem);
}

@media (max-width: 840px) {
  .settings-page__summary {
    max-width: none;
    text-align: left;
  }

  .settings-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .settings-field--wide {
    grid-column: auto;
  }

  .settings-actions {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
