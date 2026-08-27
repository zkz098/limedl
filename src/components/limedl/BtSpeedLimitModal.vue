<script setup lang="ts">
import { ref, useId, watch } from "vue";
import UiDialog from "../ui/UiDialog.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiButton from "../ui/UiButton.vue";
import { useI18n } from "../../i18n";

const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    taskId: string;
    currentDownloadLimit?: number;
    currentUploadLimit?: number;
  }>(),
  {
    currentDownloadLimit: 0,
    currentUploadLimit: 0,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [payload: { taskId: string; downloadLimit: number; uploadLimit: number }];
}>();

const { t } = useI18n();

const uid = useId();
const downloadLimitKb = ref(0);
const uploadLimitKb = ref(0);

watch(
  () => props.modelValue,
  (visible) => {
    if (visible) {
      // Reset to current values when opening (bytes → KB/s, rounded)
      downloadLimitKb.value =
        props.currentDownloadLimit > 0 ? Math.round(props.currentDownloadLimit / 1024) : 0;
      uploadLimitKb.value =
        props.currentUploadLimit > 0 ? Math.round(props.currentUploadLimit / 1024) : 0;
    }
  },
);

function handleConfirm() {
  emit("confirm", {
    taskId: props.taskId,
    downloadLimit: downloadLimitKb.value * 1024,
    uploadLimit: uploadLimitKb.value * 1024,
  });
}

function handleCancel() {
  emit("update:modelValue", false);
}
</script>

<template>
  <UiDialog
    :model-value="modelValue"
    width="min(28rem, calc(100vw - 1.5rem))"
    :close-on-overlay="false"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template #title>
      {{ t("queue.speedLimit") }}
    </template>

    <div class="speed-limit-modal">
      <div class="speed-limit-modal__field">
        <label class="speed-limit-modal__label" :for="`${uid}-download`">{{
          t("queue.btDownloadLimit")
        }}</label>
        <UiTextField
          v-model="downloadLimitKb"
          :id="`${uid}-download`"
          type="number"
          :min="0"
          placeholder="0"
          unit="KB/s"
          data-testid="bt-speed-limit-download"
        />
      </div>
      <div class="speed-limit-modal__field">
        <label class="speed-limit-modal__label" :for="`${uid}-upload`">{{
          t("queue.btUploadLimit")
        }}</label>
        <UiTextField
          v-model="uploadLimitKb"
          :id="`${uid}-upload`"
          type="number"
          :min="0"
          placeholder="0"
          unit="KB/s"
          data-testid="bt-speed-limit-upload"
        />
      </div>
      <div class="speed-limit-modal__actions">
        <UiButton variant="secondary" @click="handleCancel">
          {{ t("common.cancel") }}
        </UiButton>
        <UiButton variant="primary" @click="handleConfirm">
          {{ t("common.save") }}
        </UiButton>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.speed-limit-modal {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.speed-limit-modal__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.speed-limit-modal__label {
  font-size: var(--font-size-small);
  font-weight: 600;
  color: var(--color-text-muted);
}

.speed-limit-modal__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--color-border);
}
</style>
