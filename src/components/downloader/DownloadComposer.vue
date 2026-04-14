<script setup lang="ts">
import Button from "primevue/button";
import InputNumber from "primevue/inputnumber";
import InputText from "primevue/inputtext";
import Select from "primevue/select";

import type { ChecksumMode, DownloadFormState } from "../../types/download";

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
      <label class="field field--full">
        <span class="field__label">下载链接</span>
        <InputText
          v-model="form.url"
          type="url"
          placeholder="https://example.com/archive.iso"
        />
      </label>

      <label class="field field--full">
        <span class="field__label">保存路径</span>
        <div class="destination-field">
          <InputText
            :model-value="form.destinationDir || '选择文件夹来保存文件'"
            readonly
          />
          <Button
            type="button"
            size="small"
            severity="secondary"
            :disabled="isPickingDirectory"
            @click="$emit('pickDirectory')"
          >
            {{ isPickingDirectory ? "打开中…" : "浏览" }}
          </Button>
        </div>
      </label>

      <label class="field field--full">
        <span class="field__label">文件名（可选）</span>
        <InputText
          v-model="form.fileName"
          type="text"
          placeholder="重命名文件"
        />
      </label>

      <label class="field">
        <span class="field__label">最大连接数</span>
        <InputNumber v-model="form.maxConnections" :min="1" :use-grouping="false" />
      </label>

      <label class="field">
        <span class="field__label">重试次数</span>
        <InputNumber v-model="form.maxRetries" :min="0" :use-grouping="false" />
      </label>

      <label class="field field--full">
        <span class="field__label">校验方式</span>
        <Select
          v-model="form.checksum"
          :options="checksumOptions"
          option-label="label"
          option-value="value"
        />
      </label>

      <div class="composer-actions field--full">
        <Button type="submit" class="composer-actions__submit" :disabled="isStarting">
          {{ isStarting ? "启动中…" : "开始下载" }}
        </Button>
      </div>
    </form>
  </section>
</template>

<style scoped>
.composer-panel {
  display: grid;
  gap: var(--space-4);
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

  .field--full {
    grid-column: auto;
  }

  .destination-field {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
