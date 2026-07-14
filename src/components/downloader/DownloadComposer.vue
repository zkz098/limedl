<script setup lang="ts">
import { computed } from "vue";

import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";
import { useI18n } from "../../i18n";

import type { DownloadFormState, ThreadMode } from "../../types/download";
import type { AppSettings } from "../../types/settings";

const props = defineProps<{
  form: DownloadFormState;
  isStarting: boolean;
  isPickingDirectory: boolean;
  isPickingTorrent: boolean;
  settings: AppSettings | null;
}>();

defineEmits<{
  pickDirectory: [];
  pickTorrent: [];
  submit: [];
}>();

const { t } = useI18n();
const schedulerMode = computed(() => props.settings?.scheduler.mode ?? "automatic");
const maxThreadsPerTask = computed(
  () => props.settings?.scheduler.automatic.maxThreadsPerTask ?? 8,
);
const isBtTask = computed(() => props.form.kind === "bt");
const isHttpTask = computed(() => props.form.kind === "http");

const fixedThreadOptions = computed(() => {
  const cap = Math.max(1, maxThreadsPerTask.value);
  return [1, 2, 4, 8, 16, 32]
    .filter((value) => value <= cap)
    .map((value) => ({ label: String(value), value }));
});

const threadModeOptions = computed<Array<{ label: string; value: ThreadMode }>>(() => {
  if (schedulerMode.value === "traditional") {
    return [{ label: t("composer.fixedThreads"), value: "fixed" }];
  }

  return [
    { label: t("composer.adaptive"), value: "adaptive" },
    { label: t("composer.fixedThreads"), value: "fixed" },
  ];
});

const threadHint = computed(() => {
  if (isBtTask.value) {
    return t("composer.btHint");
  }

  if (schedulerMode.value === "traditional") {
    return t("composer.traditionalHint");
  }

  if (props.form.threadMode === "adaptive") {
    return t("composer.adaptiveHint");
  }

  return t("composer.fixedHint", { count: maxThreadsPerTask.value });
});
</script>

<template>
  <section class="composer-panel">
    <form class="composer-form" @submit.prevent="$emit('submit')">
      <section class="group field--full">
        <div class="group__head">
          <p class="section-kicker">{{ t("composer.source") }}</p>
          <h3>{{ t("composer.sourceTitle") }}</h3>
        </div>

        <div class="source-tabs">
          <button
            type="button"
            class="source-tab"
            :class="{ 'source-tab--active': form.kind === 'http' }"
            @click="form.kind = 'http'"
          >
            <span class="i-ri-links-line" aria-hidden="true" />
            <span>{{ t("composer.httpSource") }}</span>
          </button>
          <button
            type="button"
            class="source-tab"
            :class="{ 'source-tab--active': form.kind === 'bt' }"
            @click="form.kind = 'bt'"
          >
            <span class="i-ri-seedling-line" aria-hidden="true" />
            <span>{{ t("composer.btSource") }}</span>
          </button>
        </div>

        <label class="field field--full">
          <span class="field__label">{{
            isBtTask ? t("composer.torrentSource") : t("composer.url")
          }}</span>
          <div class="source-field" :class="{ 'source-field--with-picker': isBtTask }">
            <UiInput
              v-model="form.url"
              :type="isBtTask ? 'text' : 'url'"
              :placeholder="
                isBtTask
                  ? 'magnet:?xt=urn:btih:... / https://example.com/file.torrent'
                  : 'https://example.com/archive.iso'
              "
            />
            <UiButton
              v-if="isBtTask"
              type="button"
              variant="secondary"
              size="sm"
              :loading="isPickingTorrent"
              @click="$emit('pickTorrent')"
            >
              {{ isPickingTorrent ? t("common.browsing") : t("composer.chooseTorrent") }}
            </UiButton>
          </div>
        </label>

        <label v-if="isHttpTask" class="field field--full">
          <span class="field__label">{{ t("composer.fileName") }}</span>
          <UiInput
            v-model="form.fileName"
            type="text"
            :placeholder="t('composer.fileNamePlaceholder')"
          />
        </label>

        <label v-if="isHttpTask" class="field field--full">
          <span class="field__label">{{ t("composer.userAgent") }}</span>
          <UiInput
            v-model="form.userAgent"
            type="text"
            :placeholder="t('composer.userAgentPlaceholder')"
          />
          <p class="field__hint">{{ t("composer.userAgentHint") }}</p>
        </label>
      </section>

      <section class="group field--full">
        <div class="group__head">
          <p class="section-kicker">{{ t("composer.destination") }}</p>
          <h3>{{ t("composer.destinationTitle") }}</h3>
        </div>

        <label class="field field--full">
          <span class="field__label">{{ t("composer.savePath") }}</span>
          <div class="destination-field">
            <UiInput :model-value="form.destinationDir || t('composer.chooseFolder')" readonly />
            <UiButton
              type="button"
              variant="secondary"
              size="sm"
              :loading="isPickingDirectory"
              @click="$emit('pickDirectory')"
            >
              {{ isPickingDirectory ? t("common.browsing") : t("common.browse") }}
            </UiButton>
          </div>
        </label>
      </section>

      <section v-if="isHttpTask" class="group field--full group--split">
        <div class="group__head group__head--full">
          <p class="section-kicker">{{ t("composer.strategy") }}</p>
          <h3>{{ t("composer.strategyTitle") }}</h3>
        </div>

        <label class="field">
          <span class="field__label">{{ t("composer.threadStrategy") }}</span>
          <UiSelect v-model="form.threadMode" :options="threadModeOptions" />
        </label>

        <label class="field">
          <span class="field__label">{{ t("composer.threadCount") }}</span>
          <UiSelect
            v-model="form.threadCount"
            :options="fixedThreadOptions"
            :disabled="form.threadMode === 'adaptive'"
          />
        </label>

        <p class="field__hint field__hint--wide">{{ threadHint }}</p>

        <label class="field">
          <span class="field__label">{{ t("composer.retries") }}</span>
          <UiNumberField v-model="form.maxRetries" :min="0" />
        </label>
      </section>

      <section v-if="isBtTask" class="group field--full group--split">
        <div class="group__head group__head--full">
          <p class="section-kicker">{{ t("composer.strategy") }}</p>
          <h3>{{ t("composer.btStrategyTitle") }}</h3>
        </div>

        <p class="field__hint field__hint--wide">{{ threadHint }}</p>

        <label class="field">
          <span class="field__label">{{ t("composer.btDownloadLimit") }}</span>
          <UiNumberField v-model.number="form.downloadLimitBps" :min="0" />
          <p class="field__hint">{{ t("composer.btDownloadLimitHint") }}</p>
        </label>

        <label class="field">
          <span class="field__label">{{ t("composer.btUploadLimit") }}</span>
          <UiNumberField v-model.number="form.uploadLimitBps" :min="0" />
          <p class="field__hint">{{ t("composer.btUploadLimitHint") }}</p>
        </label>
      </section>

      <div class="composer-actions field--full">
        <UiButton
          type="submit"
          class="composer-actions__submit"
          block
          :loading="isStarting"
          icon="i-ri-download-2-line"
        >
          {{ isStarting ? t("composer.starting") : t("composer.start") }}
        </UiButton>
      </div>
    </form>
  </section>
</template>

<style scoped>
.composer-panel {
  display: grid;
  gap: var(--space-5);
}

.composer-form {
  display: grid;
  align-items: start;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.field {
  display: grid;
  gap: var(--space-2);
  align-content: start;
  grid-auto-rows: max-content;
}

.field--full {
  grid-column: 1 / -1;
}

.group {
  display: grid;
  align-items: start;
  grid-template-columns: inherit;
  gap: var(--space-4);
  padding: 1rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
}

.group--split {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.group__head {
  grid-column: 1 / -1;
}

.group__head h3 {
  margin: 0.15rem 0 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.source-tabs {
  grid-column: 1 / -1;
  display: inline-grid;
  grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
  gap: 0.15rem;
  padding: 0.2rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
}

.source-tab {
  min-height: 2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: var(--font-size-small);
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.source-tab:hover {
  color: var(--color-text-main);
  background: var(--color-surface-muted);
}

.source-tab--active {
  color: var(--color-accent-strong);
  border-color: var(--color-accent-soft-border);
  background: var(--color-accent-soft);
}

.field__label {
  font-size: var(--font-size-label);
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
  color: var(--color-text-muted);
}

.field__hint {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.78rem;
  line-height: 1.5;
}

.field__hint--wide {
  grid-column: 1 / -1;
  margin-top: -0.25rem;
}

.destination-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--space-2);
}

.source-field {
  display: grid;
  gap: var(--space-2);
}

.source-field--with-picker {
  grid-template-columns: minmax(0, 1fr) auto;
}

.composer-actions {
  padding-top: var(--space-1);
}

.composer-actions__submit {
  width: 100%;
}

@media (max-width: 760px) {
  .composer-form {
    grid-template-columns: minmax(0, 1fr);
  }

  .group--split {
    grid-template-columns: minmax(0, 1fr);
  }

  .field--full {
    grid-column: auto;
  }

  .destination-field {
    grid-template-columns: minmax(0, 1fr);
  }

  .source-field--with-picker {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
