<script setup lang="ts">
import { computed, ref } from "vue";
import UiButton from "../ui/UiButton.vue";
import type { AppSettings } from "../../types/settings";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
}>();

const testing = ref(false);

const statusType = computed<"idle" | "testing" | "ready" | "error">(() => {
  const cdn = props.draft.cdnAcceleration;
  if (cdn.lastError) return "error";
  if (cdn.activeIp != null && cdn.activeSpeedMbps != null) return "ready";
  if (testing.value) return "testing";
  return "idle";
});

const statusLabel = computed(() => {
  const cdn = props.draft.cdnAcceleration;
  switch (statusType.value) {
    case "error":
      return `${props.t("settings.cdnAcceleration.statusError")}: ${cdn.lastError}`;
    case "ready":
      return `${props.t("settings.cdnAcceleration.statusReady")} (IP: ${cdn.activeIp}, ${props.t("settings.cdnAcceleration.speedMbps")}: ${cdn.activeSpeedMbps} MB/s)`;
    case "testing":
      return props.t("settings.cdnAcceleration.statusTesting");
    default:
      return props.t("settings.cdnAcceleration.statusIdle");
  }
});

const hasResult = computed(
  () => props.draft.cdnAcceleration.activeIp != null,
);

const lastTestTime = computed(() => {
  const ms = props.draft.cdnAcceleration.lastTestAtMs;
  if (ms == null) return null;
  return new Date(ms).toLocaleString();
});

function startTest() {
  testing.value = true;
  // In a real implementation, this would invoke a Tauri command.
  // The results would be written to draft.cdnAcceleration by the backend.
}

function cancelTest() {
  testing.value = false;
}

function clearResult() {
  const cdn = props.draft.cdnAcceleration;
  cdn.activeIp = null;
  cdn.activeSpeedMbps = null;
  cdn.lastTestAtMs = null;
  cdn.lastError = null;
  testing.value = false;
}
</script>

<template>
  <section class="settings-section">
    <div class="settings-section__head">
      <div>
        <p class="section-kicker">{{ t("settings.cdnAcceleration.title") }}</p>
        <h3>{{ t("settings.cdnAcceleration.description") }}</h3>
      </div>
      <span
        class="settings-section__icon i-ri-rocket-2-line"
        aria-hidden="true"
      />
    </div>

    <div class="settings-grid">
      <label class="settings-field">
        <span class="settings-field__label">{{ t("settings.cdnAcceleration.enable") }}</span>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.cdnAcceleration.enabled }"
          :aria-pressed="draft.cdnAcceleration.enabled"
          @click="draft.cdnAcceleration.enabled = !draft.cdnAcceleration.enabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.cdnAcceleration.enabled
                ? 'i-ri-checkbox-circle-fill'
                : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.cdnAcceleration.enable") }}</span>
        </button>
      </label>

      <div class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.cdnAcceleration.status") }}</span>
        <span class="settings-value" :class="`settings-value--${statusType}`">
          {{ statusLabel }}
        </span>
      </div>

      <div class="settings-field settings-field--wide">
        <span class="settings-field__label">{{ t("settings.cdnAcceleration.lastResult") }}</span>
        <template v-if="hasResult">
          <div class="settings-result">
            <div class="settings-result__row">
              <span class="settings-result__key">{{ t("settings.cdnAcceleration.bestIp") }}</span>
              <span class="settings-result__value">{{ draft.cdnAcceleration.activeIp }}</span>
            </div>
            <div class="settings-result__row">
              <span class="settings-result__key">{{ t("settings.cdnAcceleration.speedMbps") }}</span>
              <span class="settings-result__value">{{ draft.cdnAcceleration.activeSpeedMbps }} MB/s</span>
            </div>
            <div v-if="lastTestTime" class="settings-result__row">
              <span class="settings-result__key">{{ t("settings.cdnAcceleration.testedAt") }}</span>
              <span class="settings-result__value">{{ lastTestTime }}</span>
            </div>
          </div>
        </template>
        <p v-else class="settings-field__hint">{{ t("settings.cdnAcceleration.noResult") }}</p>
      </div>

      <div class="settings-field settings-field--wide">
        <div class="settings-inline-field">
          <UiButton
            type="button"
            variant="secondary"
            :disabled="testing"
            @click="startTest"
          >
            {{ testing ? t("settings.cdnAcceleration.statusTesting") : t("settings.cdnAcceleration.triggerButton") }}
          </UiButton>
          <UiButton
            v-if="testing"
            type="button"
            variant="secondary"
            @click="cancelTest"
          >
            {{ t("settings.cdnAcceleration.cancelButton") }}
          </UiButton>
          <UiButton
            v-if="statusType === 'ready'"
            type="button"
            variant="secondary"
            @click="clearResult"
          >
            {{ t("settings.cdnAcceleration.clearButton") }}
          </UiButton>
        </div>
        <p class="settings-field__hint">{{ t("settings.cdnAcceleration.dataWarning") }}</p>
      </div>
    </div>
  </section>
</template>
