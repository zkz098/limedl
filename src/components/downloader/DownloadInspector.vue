<script setup lang="ts">
import { computed } from "vue";

import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import {
  formatTokenLabel,
  formatBytes,
  formatEta,
  formatSpeed,
  formatTimestamp,
  progressValue,
  stateLabel,
} from "../../lib/download-format";
import type { DownloadSnapshot, DownloadSummary } from "../../types/download";

const props = defineProps<{
  actionName: string;
  canCancel: boolean;
  canPause: boolean;
  canResume: boolean;
  isRefreshingStatus: boolean;
  selectedOverview: DownloadSummary | DownloadSnapshot | null;
  selectedSnapshot: DownloadSnapshot | null;
}>();

defineEmits<{
  cancel: [];
  pause: [];
  refresh: [];
  resume: [];
  close: [];
}>();

const detailRows = computed(() => {
  const snapshot = props.selectedSnapshot;

  if (!snapshot) {
    return [];
  }

  return [
    { label: "下载链接", value: snapshot.url, wide: true },
    { label: "最终链接", value: snapshot.finalUrl, wide: true },
    { label: "保存路径", value: snapshot.destinationPath, wide: true },
    { label: "临时文件", value: snapshot.tempPath, wide: true },
    { label: "断点续传", value: snapshot.supportsRanges ? "支持" : "不支持" },
    { label: "当前线程数", value: String(snapshot.connectionCount) },
    { label: "线程策略", value: formatTokenLabel(snapshot.threadMode) },
    {
      label: "请求线程数",
      value: snapshot.requestedThreadCount ? String(snapshot.requestedThreadCount) : "—",
    },
    {
      label: "目标线程数",
      value: snapshot.desiredThreadCount ? String(snapshot.desiredThreadCount) : "—",
    },
    {
      label: "分配线程数",
      value: snapshot.allocatedThreadCount ? String(snapshot.allocatedThreadCount) : "—",
    },
    {
      label: "自适应模式",
      value: snapshot.adaptiveProfile ? formatTokenLabel(snapshot.adaptiveProfile) : "—",
    },
    { label: "线程说明", value: snapshot.threadNote ?? "—", wide: true },
    { label: "校验方式", value: formatTokenLabel(snapshot.checksumMode) },
    { label: "校验码", value: snapshot.checksum ?? "—" },
    { label: "ETag", value: snapshot.etag ?? "—" },
    { label: "最后修改时间", value: snapshot.lastModified ?? "—" },
    { label: "创建时间", value: formatTimestamp(snapshot.createdAtMs) },
    { label: "更新时间", value: formatTimestamp(snapshot.updatedAtMs) },
  ];
});

const stateTone = computed<"neutral" | "info" | "success" | "warning" | "danger">(() => {
  const state = props.selectedOverview?.state;

  if (!state) return "neutral";
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
});
</script>

<template>
  <section class="inspector-panel">
    <div class="inspector-header">
      <div>
        <p class="section-kicker">Inspector</p>
        <h2 class="panel-title">任务详情</h2>
      </div>
      <div class="inspector-actions">
        <UiButton
          type="button"
          size="sm"
          variant="secondary"
          icon="i-ri-refresh-line"
          @click="$emit('refresh')"
        >
          {{ isRefreshingStatus ? "刷新中…" : "刷新" }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-pause-line"
          :disabled="!canPause"
          @click="$emit('pause')"
        >
          {{ actionName === "Pause" ? "暂停中…" : "暂停" }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-play-line"
          :disabled="!canResume"
          @click="$emit('resume')"
        >
          {{ actionName === "Resume" ? "恢复中…" : "恢复" }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="danger"
          icon="i-ri-close-circle-line"
          :disabled="!canCancel"
          @click="$emit('cancel')"
        >
          {{ actionName === "Cancel" ? "取消中…" : "取消" }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-close-line"
          @click="$emit('close')"
        />
      </div>
    </div>

    <div v-if="selectedOverview" class="inspector-content">
      <div class="inspector-summary">
        <div class="inspector-summary__copy">
          <div class="inspector-summary__header">
            <h3>{{ selectedOverview.fileName }}</h3>
            <UiBadge :tone="stateTone">{{ stateLabel(selectedOverview.state) }}</UiBadge>
          </div>
          <p>{{ selectedOverview.destinationPath }}</p>
        </div>

        <div class="metric-grid">
          <div class="text-row">
            <span class="text-label">已传输:</span>
            <span class="text-value">
              {{ formatBytes(selectedOverview.downloadedBytes) }} /
              {{ formatBytes(selectedOverview.totalBytes) }}
            </span>
          </div>
          <div class="text-row">
            <span class="text-label">速度:</span>
            <span class="text-value">{{ formatSpeed(selectedOverview.speedBytesPerSecond) }}</span>
          </div>
          <div class="text-row">
            <span class="text-label">剩余时间:</span>
            <span class="text-value">{{ formatEta(selectedOverview.etaSeconds) }}</span>
          </div>
          <div class="text-row">
            <span class="text-label">线程:</span>
            <span class="text-value">
              {{ selectedOverview.connectionCount }}
              <template v-if="selectedOverview.threadMode === 'adaptive'"> / 自适应 </template>
            </span>
          </div>
        </div>

        <div class="summary-progress">
          <div class="summary-progress__copy">
            <span>进度</span>
            <span>{{ progressValue(selectedOverview).toFixed(1) }}%</span>
          </div>
          <UiProgress :value="progressValue(selectedOverview)" />
        </div>
      </div>

      <dl class="detail-grid" v-if="detailRows.length">
        <div
          v-for="row in detailRows"
          :key="row.label"
          class="text-row"
          :class="{ 'text-row--wide': row.wide }"
        >
          <dt class="text-label">{{ row.label }}:</dt>
          <dd class="text-value">{{ row.value }}</dd>
        </div>
      </dl>

      <p v-if="selectedOverview.error" class="status-banner status-banner--error">
        {{ selectedOverview.error }}
      </p>
    </div>

    <div v-else class="inspector-empty">
      <h3>未选择任务</h3>
      <p>请选择一个任务以查看进度和详情。</p>
    </div>
  </section>
</template>

<style scoped>
.inspector-panel {
  display: grid;
  gap: 0.75rem;
  padding: 0.85rem 1rem;
}

.inspector-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.inspector-actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.inspector-content,
.inspector-summary {
  display: grid;
  gap: 0.75rem;
}

.inspector-summary {
  gap: 0.65rem;
}

.inspector-summary__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.inspector-summary__copy h3,
.inspector-summary__copy p,
.inspector-empty h3,
.inspector-empty p {
  margin: 0;
}

.inspector-summary__copy p,
.inspector-empty p {
  color: var(--color-text-muted);
  font-size: 0.84rem;
}

.inspector-summary__copy h3,
.inspector-empty h3 {
  color: var(--color-heading);
  font-size: 1rem;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.35rem 0.9rem;
}

.text-row {
  display: flex;
  gap: 0.45rem;
  align-items: flex-start;
  min-width: 0;
}

.text-label {
  color: var(--color-text-muted);
  font-weight: 500;
  white-space: nowrap;
  font-size: 0.7rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.text-value {
  color: var(--color-text-main);
  word-break: break-all;
  font-size: 0.84rem;
  line-height: 1.45;
}

.summary-progress {
  display: grid;
  gap: 0.35rem;
}

.summary-progress__copy {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--color-text-muted);
  font-size: 0.76rem;
}

.detail-grid {
  margin: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.35rem 0.9rem;
  padding-top: 0.35rem;
  border-top: 1px solid var(--color-border);
}

.text-row--wide {
  grid-column: 1 / -1;
}

.detail-grid dt {
  margin: 0;
}

.detail-grid dd {
  margin: 0;
}

.inspector-empty {
  display: grid;
  gap: var(--space-2);
  min-height: 14rem;
  place-content: center;
  text-align: center;
}

@media (max-width: 760px) {
  .metric-grid,
  .detail-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .text-row--wide {
    grid-column: auto;
  }
}
</style>
