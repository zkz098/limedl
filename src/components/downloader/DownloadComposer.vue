<script setup lang="ts">
import UiButton from "../ui/UiButton.vue";
import UiInput from "../ui/UiInput.vue";
import UiNumberField from "../ui/UiNumberField.vue";
import UiSelect from "../ui/UiSelect.vue";

import type { ChecksumMode, DownloadFormState } from "../../types/download";

const connectionOptions = [
  { label: "1", value: 1 },
  { label: "2", value: 2 },
  { label: "4", value: 4 },
  { label: "8", value: 8 },
  { label: "16", value: 16 },
  { label: "32", value: 32 },
];

defineProps<{
  form: DownloadFormState;
  isStarting: boolean;
  isPickingDirectory: boolean;
  checksumOptions: { label: string; value: ChecksumMode }[];
}>();

defineEmits<{
  pickDirectory: [];
  submit: [];
}>();
</script>

<template>
  <section class="composer-panel">
    <form class="composer-form" @submit.prevent="$emit('submit')">
      <section class="group field--full">
        <div class="group__head">
          <p class="section-kicker">Source</p>
          <h3>下载来源</h3>
        </div>

        <label class="field field--full">
          <span class="field__label">下载链接</span>
          <UiInput v-model="form.url" type="url" placeholder="https://example.com/archive.iso" />
        </label>

        <label class="field field--full">
          <span class="field__label">文件名（可选）</span>
          <UiInput v-model="form.fileName" type="text" placeholder="重命名文件" />
        </label>
      </section>

      <section class="group field--full">
        <div class="group__head">
          <p class="section-kicker">Destination</p>
          <h3>保存位置</h3>
        </div>

        <label class="field field--full">
          <span class="field__label">保存路径</span>
          <div class="destination-field">
            <UiInput :model-value="form.destinationDir || '选择文件夹来保存文件'" readonly />
            <UiButton
              type="button"
              variant="secondary"
              size="sm"
              :loading="isPickingDirectory"
              @click="$emit('pickDirectory')"
            >
              {{ isPickingDirectory ? "打开中…" : "浏览" }}
            </UiButton>
          </div>
        </label>
      </section>

      <section class="group field--full group--split">
        <div class="group__head group__head--full">
          <p class="section-kicker">Transfer Strategy</p>
          <h3>连接与校验</h3>
        </div>

        <label class="field">
          <span class="field__label">最大连接数</span>
          <UiSelect v-model="form.maxConnections" :options="connectionOptions" />
        </label>

        <label class="field">
          <span class="field__label">重试次数</span>
          <UiNumberField v-model="form.maxRetries" :min="0" />
        </label>

        <label class="field field--full">
          <span class="field__label">校验方式</span>
          <UiSelect v-model="form.checksum" :options="checksumOptions" />
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
          {{ isStarting ? "启动中…" : "开始下载" }}
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
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.field {
  display: grid;
  gap: var(--space-2);
}

.field--full {
  grid-column: 1 / -1;
}

.group {
  display: grid;
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
  margin: 0.25rem 0 0;
  font-size: 1rem;
  color: var(--color-heading);
}

.field__label {
  font-size: var(--font-size-label);
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
  color: var(--color-text-muted);
}

.destination-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--space-2);
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
}
</style>
