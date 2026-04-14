<script setup lang="ts">
import { computed } from "vue";
import Button from "primevue/button";
import ProgressBar from "primevue/progressbar";

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
    { label: "连接数", value: String(snapshot.connectionCount) },
    { label: "校验方式", value: formatTokenLabel(snapshot.checksumMode) },
    { label: "校验码", value: snapshot.checksum ?? "—" },
    { label: "ETag", value: snapshot.etag ?? "—" },
    { label: "最后修改时间", value: snapshot.lastModified ?? "—" },
    { label: "创建时间", value: formatTimestamp(snapshot.createdAtMs) },
    { label: "更新时间", value: formatTimestamp(snapshot.updatedAtMs) },
  ];
});
</script>

<template>
  <section class="inspector-panel">
    <div class="inspector-header">
      <div class="inspector-actions">
        <Button type="button" size="small" severity="secondary" @click="$emit('refresh')">
          {{ isRefreshingStatus ? "刷新中…" : "刷新" }}
        </Button>
        <Button type="button" size="small" text :disabled="!canPause" @click="$emit('pause')">
          {{ actionName === "Pause" ? "暂停中…" : "暂停" }}
        </Button>
        <Button type="button" size="small" text :disabled="!canResume" @click="$emit('resume')">
          {{ actionName === "Resume" ? "恢复中…" : "恢复" }}
        </Button>
        <Button type="button" size="small" severity="danger" text :disabled="!canCancel" @click="$emit('cancel')">
          {{ actionName === "Cancel" ? "取消中…" : "取消" }}
        </Button>
        <Button type="button" size="small" text icon="pi pi-times" aria-label="Close" @click="$emit('close')" />
      </div>
    </div>

    <div v-if="selectedOverview" class="inspector-content">
      <div class="inspector-summary">
        <div class="inspector-summary__copy">
          <div class="inspector-summary__header">
            <h3>{{ selectedOverview.fileName }}</h3>
            <span class="state-pill" :data-state="selectedOverview.state">
              {{ stateLabel(selectedOverview.state) }}
            </span>
          </div>
          <p>{{ selectedOverview.destinationPath }}</p>
        </div>

        <div class="metric-grid">
          <div class="text-item">
            <span class="text-label">已传输:</span>
            <span class="text-value">
              {{ formatBytes(selectedOverview.downloadedBytes) }} / {{ formatBytes(selectedOverview.totalBytes) }}
            </span>
          </div>
          <div class="text-item">
            <span class="text-label">速度:</span>
            <span class="text-value">{{ formatSpeed(selectedOverview.speedBytesPerSecond) }}</span>
          </div>
          <div class="text-item">
            <span class="text-label">剩余时间:</span>
            <span class="text-value">{{ formatEta(selectedOverview.etaSeconds) }}</span>
          </div>
          <div class="text-item">
            <span class="text-label">连接数:</span>
            <span class="text-value">{{ selectedOverview.connectionCount }}</span>
          </div>
        </div>

        <div class="summary-progress">
          <div class="summary-progress__copy">
            <span>进度</span>
            <span>{{ progressValue(selectedOverview).toFixed(1) }}%</span>
          </div>
          <ProgressBar :value="progressValue(selectedOverview)" :show-value="false" />
        </div>
      </div>

      <dl class="detail-grid" v-if="detailRows.length">
        <div
          v-for="row in detailRows"
          :key="row.label"
          class="text-item"
          :class="{ 'text-item--wide': row.wide }"
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
  gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  background: var(--color-bg-panel);
}

.inspector-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: var(--border-width-thin) solid var(--color-border);
  padding-bottom: var(--space-3);
  margin-bottom: var(--space-2);
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
  gap: var(--space-4);
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
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.text-item {
  display: flex;
  gap: var(--space-2);
  align-items: baseline;
}

.text-label {
  color: var(--color-text-muted);
  font-weight: 500;
  white-space: nowrap;
}

.text-value {
  color: var(--color-text-main);
  word-break: break-all;
}

.summary-progress {
  display: grid;
  gap: var(--space-2);
}

.summary-progress__copy {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

.detail-grid {
  margin: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.text-item--wide {
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
  min-height: var(--empty-panel-height);
  place-content: center;
  text-align: center;
}

@media (max-width: 760px) {
  .metric-grid,
  .detail-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .text-item--wide {
    grid-column: auto;
  }
}
</style>
