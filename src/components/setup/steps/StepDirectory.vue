<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../../../i18n";
import type { AppSettings } from "../../../types/settings";
import UiButton from "../../ui/UiButton.vue";
import UiTextField from "../../ui/UiTextField.vue";

const props = defineProps<{
  settings: AppSettings;
}>();

const emit = defineEmits<{
  "update:settings": [settings: AppSettings];
}>();

const { t } = useI18n();

function updateDirectory(defaultDownloadDir: string) {
  emit("update:settings", {
    ...props.settings,
    download: { ...props.settings.download, defaultDownloadDir },
  });
}

function onPathChange(value: string | number | null) {
  updateDirectory(value === null || value === undefined ? "" : String(value));
}

async function browseDirectory() {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
      title: t("setupWizard.directoryTitle"),
    });

    if (typeof selectedPath === "string") {
      updateDirectory(selectedPath);
    } else if (Array.isArray(selectedPath) && selectedPath[0]) {
      updateDirectory(selectedPath[0]);
    }
  } catch (err) {
    console.error("Directory dialog failed:", err);
  }
}
</script>

<template>
  <div class="setup-step">
    <div class="setup-step__header">
      <span class="setup-step__icon i-ri-folder-download-line" aria-hidden="true" />
      <h2 class="setup-step__title">{{ t("setupWizard.directoryTitle") }}</h2>
    </div>
    <p class="setup-step__description">{{ t("setupWizard.directoryDescription") }}</p>
    <div class="setup-step__body">
      <div class="directory-field">
        <UiTextField
          class="directory-field__input"
          type="text"
          :model-value="settings.download.defaultDownloadDir"
          :placeholder="t('setupWizard.directoryPlaceholder')"
          @update:model-value="onPathChange"
        />
        <UiButton variant="secondary" icon="i-ri-folder-open-line" @click="browseDirectory">
          {{ t("setupWizard.directoryBrowse") }}
        </UiButton>
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

.directory-field {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.directory-field__input {
  flex: 1;
  min-width: 0;
}
</style>
