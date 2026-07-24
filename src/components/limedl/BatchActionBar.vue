<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../i18n";
import UiButton from "../ui/UiButton.vue";

const props = defineProps<{
  selectedCount: number;
  multiSelectMode: boolean;
  canPauseCount: number;
  canResumeCount: number;
  canCancelCount: number;
}>();

const emit = defineEmits<{
  pause: [];
  resume: [];
  cancel: [];
  clearSelection: [];
}>();

const { t } = useI18n();

const isVisible = computed(() => props.multiSelectMode && props.selectedCount > 0);
const selectedLabel = computed(() => t("toolbar.selectedCount", { count: props.selectedCount }));
</script>

<template>
  <Transition name="batch-bar">
    <div v-if="isVisible" class="batch-action-bar" role="toolbar" :aria-label="selectedLabel">
      <span class="batch-action-bar__count">{{ selectedLabel }}</span>
      <div class="batch-action-bar__actions">
        <UiButton
          size="sm"
          variant="secondary"
          icon="i-ri-pause-line"
          :disabled="canPauseCount === 0"
          @click="emit('pause')"
        >
          <span class="batch-action-bar__label">{{ t("queue.pause") }}</span>
        </UiButton>
        <UiButton
          size="sm"
          variant="secondary"
          icon="i-ri-play-line"
          :disabled="canResumeCount === 0"
          @click="emit('resume')"
        >
          <span class="batch-action-bar__label">{{ t("queue.continue") }}</span>
        </UiButton>
        <UiButton
          size="sm"
          variant="danger"
          icon="i-ri-close-circle-line"
          :disabled="canCancelCount === 0"
          @click="emit('cancel')"
        >
          <span class="batch-action-bar__label">{{ t("queue.cancelSelected") }}</span>
        </UiButton>
        <UiButton
          size="sm"
          variant="ghost"
          icon="i-ri-close-line"
          :title="t('toolbar.deselectAll')"
          @click="emit('clearSelection')"
        />
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.batch-action-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  margin: 0 var(--space-4) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
  box-shadow: var(--shadow-card);
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.batch-action-bar__count {
  color: var(--color-accent-strong);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  white-space: nowrap;
}

.batch-action-bar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.batch-bar-enter-active,
.batch-bar-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.batch-bar-enter-from,
.batch-bar-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

@media (max-width: 680px) {
  .batch-action-bar {
    flex-wrap: wrap;
  }

  .batch-action-bar__label {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .batch-action-bar,
  .batch-bar-enter-active,
  .batch-bar-leave-active {
    transition: none;
  }
}
</style>
