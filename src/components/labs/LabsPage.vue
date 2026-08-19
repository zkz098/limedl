<script setup lang="ts">
import { ref, toRef } from "vue";

import { useI18n } from "../../i18n";
import { useNotificationStore } from "../../stores/notification";
import { saveAppSettings } from "../../lib/tauri/settings-api";
import type { AppSettings } from "../../types/settings";
import UiButton from "../ui/UiButton.vue";

import LabsCdnAccelerationPanel from "./LabsCdnAccelerationPanel.vue";
import LabsGitHubMirrorPanel from "./LabsGitHubMirrorPanel.vue";

import { serializeSettings, useSettingsForm } from "../settings/settingsComposables";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  saved: [settings: AppSettings];
  dirtyChange: [isDirty: boolean];
}>();

const { t } = useI18n();
const { notifySuccess, notifyError } = useNotificationStore();

// ── Reactive form (shared composable) ─────────────────────────────

const { form, buildSettingsPayload, savedSettingsSnapshot } = useSettingsForm({
  settings: toRef(props, "settings"),
  onDirtyChange: (isDirty) => emit("dirtyChange", isDirty),
});

// ── State ────────────────────────────────────────────────────────

const isSaving = ref(false);

// ── Actions ────────────────────────────────────────────────────────

async function persistSettings(): Promise<boolean> {
  if (isSaving.value) {
    return false;
  }

  isSaving.value = true;

  try {
    const saved = await saveAppSettings(buildSettingsPayload());

    savedSettingsSnapshot.value = serializeSettings(saved);
    emit("saved", saved);
    emit("dirtyChange", false);
    notifySuccess(t("settings.notifications.saved"));
    return true;
  } catch (error) {
    notifyError(error instanceof Error ? error.message : t("settings.notifications.saveFailed"));
    return false;
  } finally {
    isSaving.value = false;
  }
}

// ── Tabs ──────────────────────────────────────────────────────────

const activeTab = ref("cdnAcceleration");

const tabs = [
  { id: "cdnAcceleration", icon: "i-ri-speed-up-line", labelKey: "settings.cdnAcceleration.title" },
  { id: "githubMirror", icon: "i-ri-github-line", labelKey: "settings.githubMirror.title" },
] as const;

defineExpose({
  persistSettings,
});
</script>

<template>
  <section class="labs-page">
    <div class="desk-panel__header labs-page__header">
      <div>
        <p class="section-kicker">{{ t("labs.kicker") }}</p>
        <h2 class="panel-title">{{ t("labs.title") }}</h2>
      </div>
    </div>

    <div class="labs-page__warning" role="alert">
      <span class="labs-page__warning-icon i-ri-error-warning-line" aria-hidden="true" />
      <span>{{ t("labs.warning") }}</span>
    </div>

    <div class="settings-page__layout">
      <aside class="settings-page__sidebar" role="tablist" :aria-label="t('labs.title')">
        <nav class="settings-page__tabs">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            type="button"
            role="tab"
            class="settings-page__tab"
            :class="{ 'settings-page__tab--active': activeTab === tab.id }"
            :aria-selected="activeTab === tab.id"
            @click="activeTab = tab.id"
          >
            <span :class="tab.icon" aria-hidden="true" />
            <span>{{ t(tab.labelKey) }}</span>
          </button>
        </nav>

        <div class="settings-page__save">
          <p class="settings-page__save-hint">{{ t("settings.saveHint") }}</p>
          <UiButton
            type="button"
            icon="i-ri-save-line"
            block
            :loading="isSaving"
            @click="persistSettings"
          >
            {{ isSaving ? t("common.saving") : t("common.save") }}
          </UiButton>
        </div>
      </aside>

      <div class="settings-page__content">
        <LabsCdnAccelerationPanel v-show="activeTab === 'cdnAcceleration'" v-model:draft="form" :t="t" />
        <LabsGitHubMirrorPanel v-show="activeTab === 'githubMirror'" v-model:draft="form" :t="t" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.labs-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.labs-page__header {
  align-items: flex-end;
}

.labs-page__warning {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  background: var(--color-warning-bg);
  color: var(--color-warning-text);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  line-height: var(--leading-snug);
}

.labs-page__warning-icon {
  flex-shrink: 0;
  font-size: var(--text-lg);
}
</style>
