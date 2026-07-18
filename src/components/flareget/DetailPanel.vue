<script setup lang="ts">
import { computed, ref } from "vue";

import DownloadInspector from "./DownloadInspector.vue";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import { useI18n } from "../../i18n";
import type { DownloadSnapshot, DownloadSummary } from "../../types/download";

const { t } = useI18n();

const props = defineProps<{
  selectedOverview: DownloadSummary | null;
  selectedSnapshot: DownloadSnapshot | null;
  selectedId: string | null;
  canPause: boolean;
  canResume: boolean;
  canCancel: boolean;
  actionName: string;
  isRefreshingStatus: boolean;
  showDetailInfo: boolean;
}>();

const emit = defineEmits<{
  close: [];
  refresh: [];
  pause: [];
  resume: [];
  cancel: [];
}>();

const detailCollapsed = ref(false);

const stateTone = computed<"neutral" | "info" | "success" | "warning" | "danger">(() => {
  const state = props.selectedOverview?.state;
  if (!state) return "neutral";
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
});

function toggleCollapse() {
  detailCollapsed.value = !detailCollapsed.value;
}
</script>

<template>
  <div class="detail-panel" :class="{ collapsed: detailCollapsed }">
    <div class="detail-panel__header" @click="toggleCollapse">
      <div class="detail-panel__title">
        <i
          class="detail-panel__arrow"
          :class="detailCollapsed ? 'i-ri-arrow-up-line' : 'i-ri-arrow-down-line'"
        />
        <span class="detail-panel__filename">{{
          selectedOverview ? selectedOverview.fileName : t("detail.noSelection")
        }}</span>
        <template v-if="selectedOverview">
          <UiBadge :tone="stateTone" size="sm">{{ t(`states.${selectedOverview.state}`) }}</UiBadge>
          <UiBadge
            v-if="selectedOverview.cdnAccelerated"
            tone="warning"
            size="sm"
            class="detail-panel__cdn"
          >
            <span class="i-ri-flashlight-fill" aria-hidden="true" />
            {{ t("inspector.cdnAccelerated") }}
          </UiBadge>
        </template>
      </div>
      <div v-if="selectedOverview" class="detail-panel__actions">
        <UiButton
          type="button"
          size="sm"
          variant="secondary"
          icon="i-ri-refresh-line"
          @click.stop="$emit('refresh')"
        >
          {{ isRefreshingStatus ? t("common.refreshing") : t("common.refresh") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-pause-line"
          :disabled="!canPause"
          @click.stop="$emit('pause')"
        >
          {{ actionName === "Pause" ? t("inspector.pausing") : t("inspector.pause") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-play-line"
          :disabled="!canResume"
          @click.stop="$emit('resume')"
        >
          {{ actionName === "Resume" ? t("inspector.resuming") : t("inspector.resume") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="danger"
          icon="i-ri-close-circle-line"
          :disabled="!canCancel"
          @click.stop="$emit('cancel')"
        >
          {{ actionName === "Cancel" ? t("inspector.canceling") : t("inspector.cancel") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-close-line"
          @click.stop="$emit('close')"
        />
      </div>
    </div>
    <div v-show="!detailCollapsed" class="detail-panel__body">
      <DownloadInspector
        v-if="selectedOverview"
        :selected-overview="selectedOverview"
        :selected-snapshot="selectedSnapshot"
        :show-detail-info="showDetailInfo"
      />
      <div v-else class="detail-panel__empty">
        <i class="i-ri-cursor-line" />
        <p>{{ t("detail.selectPrompt") }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.detail-panel {
  flex-shrink: 0;
  border-top: 1px solid var(--color-border);
  background: var(--color-panel);
  max-height: 40vh;
  display: flex;
  flex-direction: column;
  transition: max-height 0.2s ease;
}

.detail-panel.collapsed {
  max-height: 3.75rem;
}

.detail-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  cursor: pointer;
  user-select: none;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.detail-panel.collapsed .detail-panel__header {
  border-bottom: none;
}

.detail-panel__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  min-width: 0;
  font-size: 0.875rem;
  font-weight: 500;
  color: var(--color-text-main);
}

.detail-panel__arrow {
  color: var(--color-accent);
  flex-shrink: 0;
}

.detail-panel__filename {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.detail-panel__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  flex-wrap: wrap;
}

.detail-panel__cdn {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
}

.detail-panel__cdn .i-ri-flashlight-fill {
  font-size: 0.8rem;
}

.detail-panel__body {
  flex: 1;
  overflow: auto;
  min-height: 0;
}

.detail-panel__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  color: var(--color-text-soft);
  gap: var(--space-2);
}

.detail-panel__empty i {
  font-size: 1.5rem;
}
</style>
