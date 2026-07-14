<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";

import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";
import { useI18n } from "../../i18n";

import type { ChecksumMode, DownloadFormState, TaskKind, ThreadMode } from "../../types/download";
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

const urlInputRef = ref<InstanceType<typeof UiInput> | null>(null);
const isAdvancedOpen = ref(false);
const urlError = ref("");

const schedulerMode = computed(() => props.settings?.scheduler.mode ?? "automatic");
const maxThreadsPerTask = computed(
  () => props.settings?.scheduler.automatic.maxThreadsPerTask ?? 8,
);
const isBtTask = computed(() => props.form.kind === "bt");

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

  if (props.form.threadMode === "adaptive") {
    return t("composer.adaptiveHint");
  }

  return t("composer.fixedHint", { count: maxThreadsPerTask.value });
});

function detectKindFromUrl(url: string): TaskKind {
  const trimmed = url.trim().toLowerCase();

  if (trimmed.startsWith("magnet:")) {
    return "bt";
  }

  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return trimmed.endsWith(".torrent") ? "bt" : "http";
  }

  if (/^[0-9a-f]{40}$/i.test(trimmed) || trimmed.startsWith("xt=urn:btih:")) {
    return "bt";
  }

  return "http";
}

function extractFileNameFromUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) {
    return "";
  }

  if (trimmed.toLowerCase().startsWith("magnet:")) {
    const queryIndex = trimmed.indexOf("?");
    const query = queryIndex >= 0 ? trimmed.slice(queryIndex + 1) : "";
    const dn = new URLSearchParams(query).get("dn");
    return dn ? decodeURIComponent(dn) : "";
  }

  try {
    const parsed = new URL(trimmed);
    const segment = parsed.pathname.split("/").pop();
    return segment ? decodeURIComponent(segment) : "";
  } catch {
    return "";
  }
}

function toggleKind() {
  props.form.kind = props.form.kind === "http" ? "bt" : "http";
}

function isValidUrl(url: string): boolean {
  try {
    return Boolean(new URL(url));
  } catch {
    return false;
  }
}

function validateUrl() {
  const url = props.form.url.trim();
  urlError.value = "";

  if (!url) {
    return;
  }

  const lower = url.toLowerCase();

  if (props.form.kind === "bt") {
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
  }
}

watch(
  () => props.form.url,
  (url) => {
    const detected = detectKindFromUrl(url);
    if (props.form.kind !== detected) {
      props.form.kind = detected;
    }

    if (!props.form.fileName.trim()) {
      props.form.fileName = extractFileNameFromUrl(url);
    }

    if (urlError.value) {
      validateUrl();
    }
  },
);

watch(() => props.form.kind, validateUrl);

onMounted(() => {
  void nextTick(() => {
    const input = urlInputRef.value?.$el as HTMLInputElement | undefined;
    input?.focus();
  });
});
</script>

<template>
  <form class="composer-form" @submit.prevent="$emit('submit')">
    <div class="composer-scroll">
      <div class="composer-fields">
        <!-- Source URL -->
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
            <UiInput
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
            @click="$emit('pickTorrent')"
          >
            {{ isPickingTorrent ? t("common.browsing") : t("composer.chooseTorrent") }}
          </UiButton>
        </div>

        <!-- File name -->
        <label class="composer-field">
          <span class="composer-field__label">{{ t("composer.fileName") }}</span>
          <UiInput
            v-model="form.fileName"
            type="text"
            :placeholder="t('composer.fileNamePlaceholder')"
          />
        </label>

        <!-- Save path -->
        <label class="composer-field">
          <span class="composer-field__label">{{ t("composer.savePath") }}</span>
          <div class="composer-destination">
            <UiInput
              :model-value="form.destinationDir || t('composer.chooseFolder')"
              readonly
              @click="$emit('pickDirectory')"
            />
            <UiButton
              type="button"
              class="composer-destination__btn"
              variant="ghost"
              size="sm"
              :loading="isPickingDirectory"
              icon="i-ri-folder-open-line"
              :aria-label="t('common.browse')"
              @click="$emit('pickDirectory')"
            />
          </div>
        </label>

        <!-- Advanced options -->
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
                  <label class="composer-field composer-field--compact">
                    <span class="composer-field__label">{{ t("composer.threadStrategy") }}</span>
                    <UiSelect v-model="form.threadMode" :options="threadModeOptions" />
                  </label>

                  <label class="composer-field composer-field--compact">
                    <span class="composer-field__label">{{ t("composer.threadCount") }}</span>
                    <UiSelect
                      v-model="form.threadCount"
                      :options="fixedThreadOptions"
                      :disabled="form.threadMode === 'adaptive'"
                    />
                  </label>

                  <label class="composer-field composer-field--compact">
                    <span class="composer-field__label">{{ t("composer.retries") }}</span>
                    <UiNumberField v-model="form.maxRetries" :min="0" />
                  </label>

                  <label class="composer-field composer-field--compact">
                    <span class="composer-field__label">{{ t("composer.userAgent") }}</span>
                    <UiInput
                      v-model="form.userAgent"
                      type="text"
                      :placeholder="t('composer.userAgentPlaceholder')"
                    />
                  </label>

                  <label class="composer-field composer-field--compact">
                    <span class="composer-field__label">{{ t("composer.checksum") }}</span>
                    <UiSelect v-model="form.checksum" :options="checksumOptions" />
                  </label>

                  <template v-if="form.kind === 'bt'">
                    <label class="composer-field composer-field--compact">
                      <span class="composer-field__label">{{ t("composer.btDownloadLimit") }}</span>
                      <UiNumberField v-model="form.downloadLimitBps" :min="0" />
                    </label>

                    <label class="composer-field composer-field--compact">
                      <span class="composer-field__label">{{ t("composer.btUploadLimit") }}</span>
                      <UiNumberField v-model="form.uploadLimitBps" :min="0" />
                    </label>
                  </template>
                </div>

                <p class="composer-hint">{{ threadHint }}</p>
              </div>
            </div>
          </Transition>
        </div>
      </div>
    </div>

    <div class="composer-footer">
      <UiButton type="submit" block :loading="isStarting" icon="i-ri-download-2-line">
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

.composer-source :deep(.ui-input) {
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

.composer-destination :deep(.ui-input) {
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
</style>
