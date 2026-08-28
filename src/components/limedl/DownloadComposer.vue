<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";

import UiButton from "../ui/UiButton.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import { useI18n } from "../../i18n";
import { useDownloadStore } from "../../stores/download/index";
import { storeToRefs } from "pinia";
import { detectKindFromUrl, extractFileNameFromUrl } from "../../lib/url-utils";

import type { ChecksumMode, ThreadMode } from "../../types/download";
import type { AppSettings } from "../../types/settings";

const props = defineProps<{
  settings: AppSettings | null;
}>();

const emit = defineEmits<{
  submit: [];
}>();

const { t } = useI18n();
const downloadStore = useDownloadStore();

const {
  form,
  isStarting,
  isPickingDirectory,
  isPickingTorrent,
  batchMode,
  batchUrls,
  batchEntries,
  batchSubmitProgress,
} = storeToRefs(downloadStore);
const {
  pickDestinationDirectory,
  pickTorrentSourceFile,
  parseBatchUrls,
  toggleBatchMode,
  probeSha256Checksum,
} = downloadStore;

const urlInputRef = ref<InstanceType<typeof UiTextField> | null>(null);
const isAdvancedOpen = ref(false);
const urlError = ref("");

const schedulerMode = computed(() => props.settings?.scheduler.mode ?? "automatic");
const maxThreadsPerTask = computed(
  () => props.settings?.scheduler.automatic.maxThreadsPerTask ?? 8,
);
const isBtTask = computed(() => form.value.kind === "bt");

const protocolLabel = computed(() => (isBtTask.value ? t("tokens.bt") : t("tokens.http")));
const protocolIcon = computed(() => (isBtTask.value ? "i-ri-magnet-line" : "i-ri-links-line"));

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

const checksumOptions = computed<Array<{ label: string; value: ChecksumMode }>>(() => [
  { label: t("tokens.none"), value: "none" },
  { label: t("tokens.blake3"), value: "blake3" },
  { label: t("tokens.sha256"), value: "sha256" },
  { label: t("tokens.xxh3_128"), value: "xxh3_128" },
]);

const threadHint = computed(() => {
  if (isBtTask.value) {
    return t("composer.btHint");
  }

  if (schedulerMode.value === "traditional") {
    return t("composer.traditionalHint");
  }

  if (form.value.threadMode === "adaptive") {
    return t("composer.adaptiveHint");
  }

  return t("composer.fixedHint", { count: maxThreadsPerTask.value });
});

function toggleKind() {
  // eslint-disable-next-line vue/no-mutating-props
  form.value.kind = form.value.kind === "http" ? "bt" : "http";
}

function isValidUrl(url: string): boolean {
  try {
    return Boolean(new URL(url));
  } catch {
    return false;
  }
}

function validateUrl() {
  const url = form.value.url.trim();
  urlError.value = "";

  if (!url) {
    return;
  }

  const lower = url.toLowerCase();

  if (form.value.kind === "bt") {
    if (lower.startsWith("magnet:")) {
      return;
    }

    if (lower.startsWith("http://") || lower.startsWith("https://")) {
      if (!isValidUrl(url)) {
        urlError.value = t("composer.urlInvalid");
      }
      return;
    }

    // Local file path from torrent picker — skip format validation.
    return;
  }

  if (!lower.startsWith("http://") && !lower.startsWith("https://")) {
    urlError.value = t("composer.urlInvalid");
    return;
  }

  if (!isValidUrl(url)) {
    urlError.value = t("composer.urlInvalid");
  } else if (form.value.kind === "http" && props.settings?.download.autoDetectSha256 !== false) {
    void probeSha256Checksum();
  }
}

watch(
  () => form.value.url,
  (url) => {
    const detected = detectKindFromUrl(url);
    if (form.value.kind !== detected) {
      form.value.kind = detected;
    }

    if (!form.value.fileName.trim()) {
      form.value.fileName = extractFileNameFromUrl(url);
    }

    if (urlError.value) {
      validateUrl();
    }
  },
);

watch(() => form.value.kind, validateUrl);

onMounted(() => {
  void nextTick(() => {
    const input = urlInputRef.value?.$el as HTMLInputElement | undefined;
    input?.focus();
  });
});

async function handleFormSubmit() {
  if (batchMode.value) {
    await downloadStore.submitBatch();
  } else {
    await downloadStore.submitStart();
  }
  emit("submit");
}
</script>

<template>
  <form class="composer-form" @submit.prevent="handleFormSubmit">
    <!-- Mode tabs -->
    <div class="composer-tabs" role="tablist" :aria-label="t('composer.modeLabel')">
      <button
        type="button"
        role="tab"
        class="composer-tab"
        :class="{ 'is-active': !batchMode }"
        :aria-selected="!batchMode"
        @click="batchMode ? toggleBatchMode() : undefined"
      >
        <span class="i-ri-link" aria-hidden="true" />
        <span>{{ t("composer.singleMode") }}</span>
      </button>
      <button
        type="button"
        role="tab"
        class="composer-tab"
        :class="{ 'is-active': batchMode }"
        :aria-selected="batchMode"
        @click="!batchMode ? toggleBatchMode() : undefined"
      >
        <span class="i-ri-list-check-3" aria-hidden="true" />
        <span>{{ t("composer.batchMode") }}</span>
      </button>
    </div>

    <div class="composer-scroll">
      <!-- SINGLE MODE (existing content, unchanged) -->
      <div v-if="!batchMode" class="composer-fields">
        <div class="composer-field">
          <span class="composer-field__label">{{ t("composer.sourceUrl") }}</span>
          <div
            class="composer-source"
            :class="[`composer-source--${form.kind}`, { 'is-invalid': urlError }]"
          >
            <button
              type="button"
              class="composer-protocol"
              :title="protocolLabel"
              @click="toggleKind"
            >
              <span class="composer-protocol__icon" :class="protocolIcon" aria-hidden="true" />
              <span class="composer-protocol__text">{{ protocolLabel }}</span>
            </button>
            <UiTextField
              ref="urlInputRef"
              v-model="form.url"
              type="text"
              inputmode="url"
              :placeholder="t('composer.sourceUrlPlaceholder')"
              @blur="validateUrl"
            />
          </div>
          <span v-if="urlError" class="composer-field__error">{{ urlError }}</span>
          <UiButton
            type="button"
            class="composer-torrent-link"
            variant="ghost"
            size="sm"
            :loading="isPickingTorrent"
            icon="i-ri-file-add-line"
            @click="pickTorrentSourceFile()"
          >
            {{ isPickingTorrent ? t("common.browsing") : t("composer.chooseTorrent") }}
          </UiButton>
        </div>

        <label class="composer-field" :for="'composer-file-name'">
          <span class="composer-field__label">{{ t("composer.fileName") }}</span>
          <UiTextField
            v-model="form.fileName"
            type="text"
            :placeholder="t('composer.fileNamePlaceholder')"
            :id="'composer-file-name'"
            :aria-label="t('composer.fileName')"
          />
        </label>

        <label class="composer-field" :for="'composer-save-path'">
          <span class="composer-field__label">{{ t("composer.savePath") }}</span>
          <div class="composer-destination">
            <UiTextField
              :model-value="form.destinationDir || t('composer.chooseFolder')"
              readonly
              :id="'composer-save-path'"
              :aria-label="t('composer.savePath')"
              @click="pickDestinationDirectory()"
            />
            <UiButton
              type="button"
              class="composer-destination__btn"
              variant="ghost"
              size="sm"
              :loading="isPickingDirectory"
              icon="i-ri-folder-open-line"
              :aria-label="t('common.browse')"
              @click="pickDestinationDirectory()"
            />
          </div>
        </label>

        <div class="composer-advanced">
          <button
            type="button"
            class="composer-advanced__trigger"
            :aria-expanded="isAdvancedOpen"
            @click="isAdvancedOpen = !isAdvancedOpen"
          >
            <span>{{ t("composer.advancedOptions") }}</span>
            <span
              class="composer-advanced__chevron i-ri-arrow-down-s-line"
              :class="{ 'is-open': isAdvancedOpen }"
              aria-hidden="true"
            />
          </button>
          <Transition name="collapse">
            <div v-show="isAdvancedOpen" class="composer-advanced__panel">
              <div class="composer-advanced__content">
                <div class="composer-grid">
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-thread-strategy'"
                  >
                    <span class="composer-field__label">{{ t("composer.threadStrategy") }}</span>
                    <UiSelect
                      v-model="form.threadMode"
                      :options="threadModeOptions"
                      :id="'composer-thread-strategy'"
                      :aria-label="t('composer.threadStrategy')"
                    />
                  </label>
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-thread-count'"
                  >
                    <span class="composer-field__label">{{ t("composer.threadCount") }}</span>
                    <UiSelect
                      v-model="form.threadCount"
                      :options="fixedThreadOptions"
                      :disabled="form.threadMode === 'adaptive'"
                      :id="'composer-thread-count'"
                      :aria-label="t('composer.threadCount')"
                    />
                  </label>
                  <label class="composer-field composer-field--compact" :for="'composer-retries'">
                    <span class="composer-field__label">{{ t("composer.retries") }}</span>
                    <UiTextField
                      type="number"
                      v-model="form.maxRetries"
                      :min="0"
                      :id="'composer-retries'"
                      :aria-label="t('composer.retries')"
                    />
                  </label>
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-user-agent'"
                  >
                    <span class="composer-field__label">{{ t("composer.userAgent") }}</span>
                    <UiTextField
                      v-model="form.userAgent"
                      type="text"
                      :placeholder="t('composer.userAgentPlaceholder')"
                      :id="'composer-user-agent'"
                      :aria-label="t('composer.userAgent')"
                    />
                  </label>
                  <label class="composer-field composer-field--compact" :for="'composer-checksum'">
                    <span class="composer-field__label">{{ t("composer.checksum") }}</span>
                    <UiSelect
                      v-model="form.checksum"
                      :options="checksumOptions"
                      :id="'composer-checksum'"
                      :aria-label="t('composer.checksum')"
                    />
                  </label>
                  <label
                    v-if="form.kind === 'http'"
                    class="composer-field composer-field--compact"
                    :for="'composer-expected-checksum'"
                  >
                    <div class="composer-field__header">
                      <span class="composer-field__label">{{
                        t("composer.expectedChecksum")
                      }}</span>
                      <span v-if="form.checksumDetected" class="composer-field__badge">
                        <span class="i-ri-check-line" aria-hidden="true" />
                        {{ t("composer.checksumDetected") }}
                      </span>
                    </div>
                    <div class="composer-checksum-input">
                      <UiTextField
                        v-model="form.expectedChecksum"
                        type="text"
                        :placeholder="t('composer.expectedChecksumPlaceholder')"
                        :id="'composer-expected-checksum'"
                        :aria-label="t('composer.expectedChecksum')"
                        @input="form.checksumDetected = false"
                      />
                      <UiButton
                        type="button"
                        variant="ghost"
                        size="sm"
                        class="composer-probe-btn"
                        :loading="form.isProbingChecksum"
                        icon="i-ri-radar-line"
                        :title="t('composer.probeChecksum')"
                        @click="probeSha256Checksum()"
                      />
                    </div>
                  </label>
                  <template v-if="form.kind === 'bt'">
                    <label
                      class="composer-field composer-field--compact"
                      :for="'composer-bt-download-limit'"
                    >
                      <span class="composer-field__label">{{ t("composer.btDownloadLimit") }}</span>
                      <UiTextField
                        type="number"
                        v-model="form.downloadLimitBps"
                        :min="0"
                        :id="'composer-bt-download-limit'"
                        :aria-label="t('composer.btDownloadLimit')"
                      />
                    </label>
                    <label
                      class="composer-field composer-field--compact"
                      :for="'composer-bt-upload-limit'"
                    >
                      <span class="composer-field__label">{{ t("composer.btUploadLimit") }}</span>
                      <UiTextField
                        type="number"
                        v-model="form.uploadLimitBps"
                        :min="0"
                        :id="'composer-bt-upload-limit'"
                        :aria-label="t('composer.btUploadLimit')"
                      />
                    </label>
                  </template>
                </div>
                <p class="composer-hint">{{ threadHint }}</p>
              </div>
            </div>
          </Transition>
        </div>
      </div>

      <!-- BATCH MODE -->
      <div v-else class="composer-fields">
        <label class="composer-field">
          <span class="composer-field__label">{{ t("composer.batchUrls") }}</span>
          <textarea
            class="composer-batch-textarea"
            :value="batchUrls"
            :placeholder="t('composer.batchPlaceholder')"
            rows="6"
            @input="batchUrls = ($event.target as HTMLTextAreaElement).value"
            @blur="parseBatchUrls()"
          ></textarea>
          <span class="composer-field__helper">{{ t("composer.batchHint") }}</span>
        </label>

        <div class="composer-batch-actions">
          <UiButton
            type="button"
            variant="ghost"
            size="sm"
            icon="i-ri-refresh-line"
            @click="parseBatchUrls()"
          >
            {{ t("composer.batchParse") }}
          </UiButton>
          <span v-if="batchEntries.length > 0" class="composer-batch-count">
            {{ t("composer.batchParsed", { count: batchEntries.length }) }}
          </span>
        </div>

        <div v-if="batchEntries.length > 0" class="composer-batch-preview">
          <div class="composer-batch-table-wrap">
            <table class="composer-batch-table">
              <thead>
                <tr>
                  <th class="col-index">#</th>
                  <th class="col-url">{{ t("composer.sourceUrl") }}</th>
                  <th class="col-type">{{ t("composer.batchType") }}</th>
                  <th class="col-name">{{ t("composer.fileName") }}</th>
                  <th class="col-status">{{ t("composer.batchStatus") }}</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(entry, i) in batchEntries"
                  :key="entry.id"
                  :class="`row-${entry.status}`"
                >
                  <td class="col-index">{{ i + 1 }}</td>
                  <td class="col-url" :title="entry.url">{{ entry.url }}</td>
                  <td class="col-type">
                    <span class="batch-kind-badge" :class="`batch-kind--${entry.kind}`">
                      {{ entry.kind === "bt" ? "BT" : "HTTP" }}
                    </span>
                  </td>
                  <td class="col-name" :title="entry.fileName">{{ entry.fileName || "\u2014" }}</td>
                  <td class="col-status">
                    <span v-if="entry.status === 'ready'" class="batch-status batch-status--ready"
                      >&#10003;</span
                    >
                    <span
                      v-else-if="entry.status === 'queued'"
                      class="batch-status batch-status--queued"
                      >{{ t("composer.batchQueued") }}</span
                    >
                    <span
                      v-else-if="entry.status === 'success'"
                      class="batch-status batch-status--success"
                      >&#10003; {{ t("composer.batchDone") }}</span
                    >
                    <span
                      v-else-if="entry.status === 'error'"
                      class="batch-status batch-status--error"
                      :title="entry.error"
                      >{{ t("composer.batchFailed") }}</span
                    >
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- Save path (shared for batch) -->
        <label class="composer-field" :for="'composer-save-path'">
          <span class="composer-field__label">{{ t("composer.savePath") }}</span>
          <div class="composer-destination">
            <UiTextField
              :model-value="form.destinationDir || t('composer.chooseFolder')"
              readonly
              :id="'composer-save-path'"
              :aria-label="t('composer.savePath')"
              @click="pickDestinationDirectory()"
            />
            <UiButton
              type="button"
              class="composer-destination__btn"
              variant="ghost"
              size="sm"
              :loading="isPickingDirectory"
              icon="i-ri-folder-open-line"
              :aria-label="t('common.browse')"
              @click="pickDestinationDirectory()"
            />
          </div>
        </label>

        <!-- Advanced options (shared for batch) — HTTP only, no BT fields -->
        <div class="composer-advanced">
          <button
            type="button"
            class="composer-advanced__trigger"
            :aria-expanded="isAdvancedOpen"
            @click="isAdvancedOpen = !isAdvancedOpen"
          >
            <span>{{ t("composer.advancedOptions") }}</span>
            <span
              class="composer-advanced__chevron i-ri-arrow-down-s-line"
              :class="{ 'is-open': isAdvancedOpen }"
              aria-hidden="true"
            />
          </button>
          <Transition name="collapse">
            <div v-show="isAdvancedOpen" class="composer-advanced__panel">
              <div class="composer-advanced__content">
                <div class="composer-grid">
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-thread-strategy'"
                  >
                    <span class="composer-field__label">{{ t("composer.threadStrategy") }}</span>
                    <UiSelect
                      v-model="form.threadMode"
                      :options="threadModeOptions"
                      :id="'composer-thread-strategy'"
                      :aria-label="t('composer.threadStrategy')"
                    />
                  </label>
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-thread-count'"
                  >
                    <span class="composer-field__label">{{ t("composer.threadCount") }}</span>
                    <UiSelect
                      v-model="form.threadCount"
                      :options="fixedThreadOptions"
                      :disabled="form.threadMode === 'adaptive'"
                      :id="'composer-thread-count'"
                      :aria-label="t('composer.threadCount')"
                    />
                  </label>
                  <label class="composer-field composer-field--compact" :for="'composer-retries'">
                    <span class="composer-field__label">{{ t("composer.retries") }}</span>
                    <UiTextField
                      type="number"
                      v-model="form.maxRetries"
                      :min="0"
                      :id="'composer-retries'"
                      :aria-label="t('composer.retries')"
                    />
                  </label>
                  <label
                    class="composer-field composer-field--compact"
                    :for="'composer-user-agent'"
                  >
                    <span class="composer-field__label">{{ t("composer.userAgent") }}</span>
                    <UiTextField
                      v-model="form.userAgent"
                      type="text"
                      :placeholder="t('composer.userAgentPlaceholder')"
                      :id="'composer-user-agent'"
                      :aria-label="t('composer.userAgent')"
                    />
                  </label>
                  <label class="composer-field composer-field--compact" :for="'composer-checksum'">
                    <span class="composer-field__label">{{ t("composer.checksum") }}</span>
                    <UiSelect
                      v-model="form.checksum"
                      :options="checksumOptions"
                      :id="'composer-checksum'"
                      :aria-label="t('composer.checksum')"
                    />
                  </label>
                </div>
                <p class="composer-hint">{{ threadHint }}</p>
              </div>
            </div>
          </Transition>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <div class="composer-footer">
      <div v-if="batchMode && batchSubmitProgress.total > 0" class="composer-batch-progress">
        <span class="composer-batch-progress__text">
          {{
            t("composer.batchProgress", {
              done: batchSubmitProgress.done,
              total: batchSubmitProgress.total,
            })
          }}
        </span>
        <div class="composer-batch-progress__bar">
          <div
            class="composer-batch-progress__fill"
            :style="{
              width:
                batchSubmitProgress.total > 0
                  ? `${(batchSubmitProgress.done / batchSubmitProgress.total) * 100}%`
                  : '0%',
            }"
          />
        </div>
      </div>
      <UiButton
        v-if="batchMode"
        type="submit"
        block
        :loading="isStarting"
        :disabled="batchEntries.length === 0"
        icon="i-ri-download-2-line"
      >
        {{
          isStarting
            ? t("composer.batchStarting")
            : t("composer.batchStart", { count: batchEntries.length })
        }}
      </UiButton>
      <UiButton v-else type="submit" block :loading="isStarting" icon="i-ri-download-2-line">
        {{ isStarting ? t("composer.starting") : t("composer.start") }}
      </UiButton>
    </div>
  </form>
</template>

<style scoped>
.composer-form {
  display: flex;
  flex-direction: column;
  max-height: min(44rem, calc(100vh - 8rem));
}

.composer-scroll {
  flex: 1 1 auto;
  overflow-y: auto;
  padding-right: var(--space-1);
}

.composer-fields {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding-bottom: var(--space-1);
}

.composer-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.composer-field--compact {
  gap: var(--space-2);
}

.composer-field__label {
  font-size: var(--font-size-small);
  font-weight: 500;
  color: var(--color-text-main);
}

.composer-field__error {
  color: var(--color-danger-text);
  font-size: var(--font-size-small);
}

.composer-source {
  --protocol-color: var(--color-accent);

  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-input-bg);
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.composer-source--bt {
  --protocol-color: var(--color-info-text);
}

.composer-source:focus-within {
  border-color: var(--protocol-color);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--protocol-color) 16%, transparent);
}

.composer-source.is-invalid {
  border-color: var(--color-danger-text);
}

.composer-source.is-invalid:focus-within {
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-danger-text) 16%, transparent);
}

.composer-source :deep(.ui-textfield) {
  flex: 1 1 auto;
  min-height: 2.5rem;
  border: none;
  background: transparent;
  box-shadow: none !important;
  padding: 0 0.75rem;
}

.composer-protocol {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.7rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: color-mix(in oklch, var(--protocol-color) 12%, transparent);
  color: var(--protocol-color);
  font-size: var(--font-size-small);
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background-color 0.2s ease,
    color 0.2s ease,
    transform 0.15s ease;
}

.composer-protocol:hover {
  background: color-mix(in oklch, var(--protocol-color) 20%, transparent);
}

.composer-protocol:active {
  transform: scale(0.96);
}

.composer-protocol__icon {
  font-size: 1rem;
}

.composer-torrent-link {
  align-self: flex-start;
  margin-top: var(--space-1);
  padding: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

.composer-torrent-link:hover:not(:disabled) {
  color: var(--color-accent-strong);
}

.composer-destination {
  position: relative;
  display: flex;
  align-items: center;
}

.composer-destination :deep(.ui-textfield) {
  padding-right: 2.75rem;
  cursor: pointer;
}

.composer-destination__btn {
  position: absolute;
  right: 0.25rem;
  top: 50%;
  transform: translateY(-50%);
}

.composer-advanced {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
  overflow: hidden;
}

.composer-advanced__trigger {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border: none;
  background: transparent;
  color: var(--color-text-main);
  font-size: var(--font-size-small);
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.composer-advanced__trigger:hover {
  background: var(--color-surface-muted);
}

.composer-advanced__chevron {
  color: var(--color-text-muted);
  transition: transform 0.25s ease;
}

.composer-advanced__chevron.is-open {
  transform: rotate(180deg);
}

.composer-advanced__panel {
  display: grid;
  grid-template-rows: 1fr;
}

.composer-advanced__content {
  overflow: hidden;
  padding: 0 var(--space-4) var(--space-4);
}

.composer-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.composer-hint {
  margin: var(--space-3) 0 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  line-height: 1.5;
}

.composer-footer {
  flex-shrink: 0;
  padding-top: var(--space-4);
  background: var(--color-panel);
}

.collapse-enter-active,
.collapse-leave-active {
  transition:
    grid-template-rows 0.25s ease,
    opacity 0.25s ease;
}

.collapse-enter-from,
.collapse-leave-to {
  grid-template-rows: 0fr;
  opacity: 0;
}

@media (max-width: 560px) {
  .composer-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

/* ── Mode tabs ──────────────────────────────────────────────────── */
.composer-tabs {
  display: flex;
  gap: 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: var(--space-4);
  flex-shrink: 0;
}

.composer-tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border: none;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  font-weight: 500;
  cursor: pointer;
  transition:
    color 0.2s ease,
    border-color 0.2s ease;
}

.composer-tab:hover {
  color: var(--color-text-main);
}

.composer-tab.is-active {
  color: var(--color-accent);
  border-bottom-color: var(--color-accent);
}

.composer-tab:focus-visible {
  outline: 2px solid var(--color-focus-ring);
  outline-offset: -2px;
  border-radius: var(--radius-sm);
}

/* ── Batch textarea ─────────────────────────────────────────────── */
.composer-batch-textarea {
  width: 100%;
  min-height: 8rem;
  padding: var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font-size: var(--font-size-small);
  font-family: var(--font-mono, "Cascadia Code", "Fira Code", "Consolas", monospace);
  line-height: 1.6;
  resize: vertical;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.composer-batch-textarea::placeholder {
  color: var(--color-text-soft);
}

.composer-batch-textarea:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-accent) 16%, transparent);
}

.composer-field__helper {
  font-size: var(--font-size-micro);
  color: var(--color-text-muted);
  margin-top: var(--space-1);
}

/* ── Batch actions row ──────────────────────────────────────────── */
.composer-batch-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.composer-batch-count {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

/* ── Batch preview table ────────────────────────────────────────── */
.composer-batch-preview {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.composer-batch-table-wrap {
  max-height: 12rem;
  overflow-y: auto;
}

.composer-batch-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-small);
}

.composer-batch-table thead {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--color-panel-muted);
}

.composer-batch-table th {
  padding: var(--space-2) var(--space-3);
  text-align: left;
  font-weight: 600;
  color: var(--color-text-muted);
  font-size: var(--font-size-micro);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  border-bottom: 1px solid var(--color-border);
  white-space: nowrap;
}

.composer-batch-table td {
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-main);
  vertical-align: middle;
}

.composer-batch-table tbody tr:last-child td {
  border-bottom: none;
}

.composer-batch-table .col-index {
  width: 2.5rem;
  text-align: center;
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.composer-batch-table .col-type {
  width: 4rem;
}

.composer-batch-table .col-status {
  width: 5rem;
  white-space: nowrap;
}

.composer-batch-table .col-url {
  max-width: 16rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.composer-batch-table .col-name {
  max-width: 10rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.composer-batch-table .row-queued {
  background: color-mix(in oklch, var(--color-accent-bg) 40%, transparent);
}

.composer-batch-table .row-error {
  background: color-mix(in oklch, var(--color-danger-bg) 40%, transparent);
}

/* ── Kind badge ─────────────────────────────────────────────────── */
.batch-kind-badge {
  display: inline-block;
  padding: 1px var(--space-2);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-micro);
  font-weight: 600;
  line-height: 1.6;
}

.batch-kind--http {
  background: color-mix(in oklch, var(--color-accent) 12%, transparent);
  color: var(--color-accent-strong);
}

.batch-kind--bt {
  background: color-mix(in oklch, var(--color-info-text) 12%, transparent);
  color: var(--color-info-text);
}

/* ── Status indicators ──────────────────────────────────────────── */
.batch-status {
  font-size: var(--font-size-micro);
}

.batch-status--ready {
  color: var(--color-text-muted);
}

.batch-status--queued {
  color: var(--color-info-text);
}

.batch-status--success {
  color: var(--color-success-text);
}

.batch-status--error {
  color: var(--color-danger-text);
}

/* ── Batch progress bar ─────────────────────────────────────────── */
.composer-batch-progress {
  margin-bottom: var(--space-3);
}

.composer-batch-progress__text {
  display: block;
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  margin-bottom: var(--space-2);
}

.composer-batch-progress__bar {
  height: 4px;
  border-radius: var(--radius-pill);
  background: var(--color-surface-muted);
  overflow: hidden;
}

.composer-batch-progress__fill {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--color-accent);
  transition: width 0.3s ease;
}

.composer-checksum-input {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.composer-checksum-input :deep(.ui-text-field) {
  flex: 1 1 auto;
}

.composer-probe-btn {
  flex-shrink: 0;
}

.composer-field__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.composer-field__badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  color: var(--color-success-text);
  font-weight: 500;
}
</style>
