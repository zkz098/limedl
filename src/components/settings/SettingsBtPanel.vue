<script setup lang="ts">
import { computed } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings, BtBackendKind } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  btSummary: string;
  btUploadLimitMiB: number;
  isFetchingTrackerList: boolean;
  defaultTrackerListUrl: string;
}>();

const emit = defineEmits<{
  "update:btUploadLimitMiB": [value: number | null];
  fetchTrackerList: [];
}>();

const trackerListEntries = computed(() =>
  props.draft.bt.trackerList
    .split(/\r?\n/)
    .map((tracker) => tracker.trim())
    .filter(Boolean),
);

const pauseEnabled = computed(() => props.draft.bt.pauseUploadWhenLimitReached);

const isOwnBackend = computed(() => props.draft.bt.backend === "own");

const backendOptions = [
  { label: props.t("settings.btBackendRqbit"), value: "rqbit" as BtBackendKind },
  { label: props.t("settings.btBackendOwn"), value: "own" as BtBackendKind },
];
</script>

<template>
  <SettingsSection :title="t('settings.btTitle')" icon="i-ri-seedling-line" :summary="btSummary">
    <SettingsField wide :label="t('settings.btBackendLabel')">
      <div class="settings-inline-field">
        <UiSelect
          v-model="draft.bt.backend"
          :options="backendOptions"
          :placeholder="t('settings.btBackendLabel')"
        />
      </div>
      <p v-if="isOwnBackend" class="settings-field__hint" style="color: var(--color-warning, #f0a030)">
        {{ t("settings.btBackendOwnWarning") }}
      </p>
    </SettingsField>

    <div class="settings-grid">
      <SettingsField :label="t('settings.btDht')" :hint="t('settings.btDhtHint')">
        <UiSwitch v-model="draft.bt.dhtEnabled" :label="t('settings.btDhtNetwork')" />
      </SettingsField>

      <SettingsField wide :label="t('settings.btTrackerListUrl')" :hint="t('settings.btTrackerListUrlHint')">
        <div class="settings-inline-field">
          <UiTextField
            v-model="draft.bt.trackerListUrl"
            type="url"
            inputmode="url"
            :placeholder="defaultTrackerListUrl"
          />
          <UiButton
            type="button"
            variant="secondary"
            size="sm"
            icon="i-ri-refresh-line"
            :loading="isFetchingTrackerList"
            @click="emit('fetchTrackerList')"
          >
            {{
              isFetchingTrackerList
                ? t("settings.btTrackerListUpdating")
                : t("settings.btTrackerListUpdate")
            }}
          </UiButton>
        </div>
      </SettingsField>

      <SettingsField wide :label="t('settings.btTrackerList')" :hint="t('settings.btTrackerListHint', { count: trackerListEntries.length })">
        <textarea
          v-model="draft.bt.trackerList"
          class="settings-textarea"
          :placeholder="t('settings.btTrackerListPlaceholder')"
          rows="5"
          spellcheck="false"
        />
      </SettingsField>

      <SettingsField wide :label="t('settings.btPauseUpload')" :hint="t('settings.btPauseUploadHint')">
        <UiSwitch
          v-model="draft.bt.pauseUploadWhenLimitReached"
          :label="t('settings.btAutoPauseUpload')"
        />
      </SettingsField>

      <SettingsField :label="t('settings.btUploadLimit')" :hint="t('settings.btUploadLimitHint')">
        <UiTextField
          type="number"
          :model-value="btUploadLimitMiB"
          :min="0"
          :max="10485760"
          :disabled="!pauseEnabled"
          unit="MiB"
          @update:model-value="emit('update:btUploadLimitMiB', $event as number | null)"
        />
      </SettingsField>

      <SettingsField :label="t('settings.btRatioLimit')" :hint="t('settings.btRatioLimitHint')">
        <UiTextField
          type="number"
          v-model="draft.bt.uploadRatioLimit"
          :min="0"
          :max="100"
          :step="0.1"
          :disabled="!pauseEnabled"
          unit="x"
        />
      </SettingsField>

      <!-- TODO M2 (Oracle): These two speed-limit fields are frontend-only for now.
           The Rust BtSettings in src-tauri/src/download/types.rs does not yet have
           default_download_speed_limit / default_upload_speed_limit fields.
           Values entered here are NOT persisted to the backend and will be lost on restart.
           Once backend support is added, also wire them into SettingsPage.vue's save payload. -->
      <SettingsField :label="t('settings.btDownloadSpeedLimit')" :hint="t('settings.btDownloadSpeedLimitHint')">
        <UiTextField
          type="number"
          :model-value="draft.bt.defaultDownloadSpeedLimit ?? null"
          :min="0"
          :step="1024"
          unit="B/s"
          @update:model-value="draft.bt.defaultDownloadSpeedLimit = Number($event ?? 0)"
        />
      </SettingsField>

      <SettingsField :label="t('settings.btUploadSpeedLimit')" :hint="t('settings.btUploadSpeedLimitHint')">
        <UiTextField
          type="number"
          :model-value="draft.bt.defaultUploadSpeedLimit ?? null"
          :min="0"
          :step="1024"
          unit="B/s"
          @update:model-value="draft.bt.defaultUploadSpeedLimit = Number($event ?? 0)"
        />
      </SettingsField>
    </div>
  </SettingsSection>
</template>
