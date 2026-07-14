<script setup lang="ts">
import { computed } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import UiUnitInput from "../ui/UiUnitInput.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings } from "../../types/settings";

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
</script>

<template>
  <UiCard>
    <template #header>
      <div class="settings-section__head">
        <div>
          <h3>{{ t("settings.btTitle") }}</h3>
        </div>
        <span class="settings-section__icon i-ri-seedling-line" aria-hidden="true" />
      </div>
    </template>

    <p class="settings-section__summary">{{ btSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btDht") }}</span>
        <UiSwitch v-model="draft.bt.dhtEnabled" :label="t('settings.btDhtNetwork')" />
        <p class="settings-field__hint">{{ t("settings.btDhtHint") }}</p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.btTrackerListUrl") }}</span>
        <div class="settings-inline-field">
          <UiInput
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
        <p class="settings-field__hint">{{ t("settings.btTrackerListUrlHint") }}</p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.btTrackerList") }}</span>
        <textarea
          v-model="draft.bt.trackerList"
          class="settings-textarea"
          :placeholder="t('settings.btTrackerListPlaceholder')"
          rows="5"
          spellcheck="false"
        />
        <p class="settings-field__hint">
          {{ t("settings.btTrackerListHint", { count: trackerListEntries.length }) }}
        </p>
      </label>

      <label class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.btPauseUpload") }}</span>
        <UiSwitch
          v-model="draft.bt.pauseUploadWhenLimitReached"
          :label="t('settings.btAutoPauseUpload')"
        />
        <p class="settings-field__hint">{{ t("settings.btPauseUploadHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btUploadLimit") }}</span>
        <UiUnitInput
          :model-value="btUploadLimitMiB"
          :min="0"
          :max="10485760"
          :disabled="!pauseEnabled"
          unit="MiB"
          @update:model-value="emit('update:btUploadLimitMiB', $event)"
        />
        <p class="settings-field__hint">{{ t("settings.btUploadLimitHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btRatioLimit") }}</span>
        <UiUnitInput
          v-model="draft.bt.uploadRatioLimit"
          :min="0"
          :max="100"
          :step="0.1"
          :disabled="!pauseEnabled"
          unit="x"
        />
        <p class="settings-field__hint">{{ t("settings.btRatioLimitHint") }}</p>
      </label>

      <!-- TODO M2 (Oracle): These two speed-limit fields are frontend-only for now.
           The Rust BtSettings in src-tauri/src/download/types.rs does not yet have
           default_download_speed_limit / default_upload_speed_limit fields.
           Values entered here are NOT persisted to the backend and will be lost on restart.
           Once backend support is added, also wire them into SettingsPage.vue's save payload. -->
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btDownloadSpeedLimit") }}</span>
        <UiUnitInput
          :model-value="draft.bt.defaultDownloadSpeedLimit ?? null"
          :min="0"
          :step="1024"
          unit="B/s"
          @update:model-value="draft.bt.defaultDownloadSpeedLimit = $event ?? 0"
        />
        <p class="settings-field__hint">{{ t("settings.btDownloadSpeedLimitHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btUploadSpeedLimit") }}</span>
        <UiUnitInput
          :model-value="draft.bt.defaultUploadSpeedLimit ?? null"
          :min="0"
          :step="1024"
          unit="B/s"
          @update:model-value="draft.bt.defaultUploadSpeedLimit = $event ?? 0"
        />
        <p class="settings-field__hint">{{ t("settings.btUploadSpeedLimitHint") }}</p>
      </label>
    </div>
  </UiCard>
</template>
