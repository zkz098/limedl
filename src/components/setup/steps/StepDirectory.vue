<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../../../i18n";
import type { AppSettings } from "../../../types/settings";
import StepShell from "../StepShell.vue";
import SettingsSection from "../../settings/SettingsSection.vue";
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
  <StepShell
    icon="i-ri-folder-download-line"
    title-key="setupWizard.directoryTitle"
    description-key="setupWizard.directoryDescription"
  >
    <SettingsSection
      :title="t('setupWizard.directoryTitle')"
      icon="i-ri-folder-3-line"
      :summary="t('setupWizard.directoryDescription')"
    >
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
    </SettingsSection>
  </StepShell>
</template>

<style scoped>
.directory-field {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.directory-field__input {
  flex: 1;
  min-width: 0;
}

@media (max-width: 680px) {
  .directory-field {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
