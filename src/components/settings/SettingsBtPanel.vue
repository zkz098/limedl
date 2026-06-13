<script setup lang="ts">
import { computed } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
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
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.bt") }}</p>
        <h3>{{ t("settings.btTitle") }}</h3>
      </div>
      <span class="settings-section__icon i-ri-seedling-line" aria-hidden="true" />
    </div>

    <p class="settings-section__summary">{{ btSummary }}</p>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btDht") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.bt.dhtEnabled }"
          :aria-pressed="draft.bt.dhtEnabled"
          @click="draft.bt.dhtEnabled = !draft.bt.dhtEnabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.bt.dhtEnabled ? 'i-ri-checkbox-circle-fill' : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.btDhtNetwork") }}</span>
        </button>
        <p class="settings-field__hint">{{ t("settings.btDhtHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btPex") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.bt.pexEnabled }"
          :aria-pressed="draft.bt.pexEnabled"
          @click="draft.bt.pexEnabled = !draft.bt.pexEnabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.bt.pexEnabled ? 'i-ri-checkbox-circle-fill' : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.btPexExchange") }}</span>
        </button>
        <p class="settings-field__hint">{{ t("settings.btPexHint") }}</p>
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
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': pauseEnabled }"
          :aria-pressed="pauseEnabled"
          @click="draft.bt.pauseUploadWhenLimitReached = !draft.bt.pauseUploadWhenLimitReached"
        >
          <span
            class="settings-toggle__icon"
            :class="pauseEnabled ? 'i-ri-checkbox-circle-fill' : 'i-ri-checkbox-blank-circle-line'"
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.btAutoPauseUpload") }}</span>
        </button>
        <p class="settings-field__hint">{{ t("settings.btPauseUploadHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btUploadLimit") }}</span>
        <UiNumberField
          :model-value="btUploadLimitMiB"
          :min="0"
          :max="10485760"
          :disabled="!pauseEnabled"
          @update:model-value="emit('update:btUploadLimitMiB', $event)"
        />
        <p class="settings-field__hint">{{ t("settings.btUploadLimitHint") }}</p>
      </label>

      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.btRatioLimit") }}</span>
        <UiNumberField
          v-model="draft.bt.uploadRatioLimit"
          :min="0"
          :max="100"
          :step="0.1"
          :disabled="!pauseEnabled"
        />
        <p class="settings-field__hint">{{ t("settings.btRatioLimitHint") }}</p>
      </label>
    </div>
  </section>
</template>
