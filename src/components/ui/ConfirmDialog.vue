<script setup lang="ts">
import UiDialog from "./UiDialog.vue";
import UiButton from "./UiButton.vue";

withDefaults(
  defineProps<{
    modelValue: boolean;
    kicker: string;
    title: string;
    message: string;
    confirmText: string;
    cancelText: string;
    width?: string;
    icon?: string;
    iconDanger?: boolean;
    confirmVariant?: "primary" | "secondary" | "danger" | "ghost";
    confirmIcon?: string;
    confirmLoading?: boolean;
    confirmDisabled?: boolean;
    cancelDisabled?: boolean;
    closeOnOverlay?: boolean;
  }>(),
  {
    width: "min(32rem, calc(100vw - 1.5rem))",
    iconDanger: false,
    confirmVariant: "danger",
    confirmLoading: false,
    confirmDisabled: false,
    cancelDisabled: false,
    closeOnOverlay: true,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [];
  cancel: [];
}>();

defineSlots<{
  default?: (props: Record<string, never>) => unknown;
  "extra-actions"?: (props: Record<string, never>) => unknown;
}>();
</script>

<template>
  <UiDialog
    :model-value="modelValue"
    :width="width"
    :close-on-overlay="closeOnOverlay"
    @update:model-value="
      (value) => {
        if (!value) emit('cancel');
      }
    "
  >
    <template #title>
      <div class="dialog-heading">
        <div>
          <p class="section-kicker">{{ kicker }}</p>
          <h2>{{ title }}</h2>
        </div>
        <span
          v-if="icon"
          class="dialog-heading__icon"
          :class="[icon, { 'dialog-heading__icon--danger': iconDanger }]"
          aria-hidden="true"
        />
      </div>
    </template>

    <div class="confirm-delete">
      <p class="confirm-delete__message">{{ message }}</p>
      <slot />
      <div class="confirm-delete__actions">
        <UiButton
          type="button"
          variant="secondary"
          :disabled="cancelDisabled"
          @click="
            emit('cancel');
            emit('update:modelValue', false);
          "
        >
          {{ cancelText }}
        </UiButton>
        <slot name="extra-actions" />
        <UiButton
          type="button"
          :variant="confirmVariant"
          :icon="confirmIcon"
          :loading="confirmLoading"
          :disabled="confirmDisabled"
          @click="emit('confirm')"
        >
          {{ confirmText }}
        </UiButton>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.dialog-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  width: 100%;
}

.dialog-heading h2 {
  margin: 0.15rem 0 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-heading);
}

.dialog-heading__icon {
  font-size: 1.25rem;
  color: var(--color-text-muted);
}

.dialog-heading__icon--danger {
  color: var(--color-danger-text);
}

.confirm-delete {
  display: grid;
  gap: 1rem;
}

.confirm-delete__message {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  line-height: 1.6;
}

.confirm-delete__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  flex-wrap: wrap;
  padding-top: 0.25rem;
}
</style>
