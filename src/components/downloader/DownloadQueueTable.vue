<script setup lang="ts">
import Button from "primevue/button";
import Column from "primevue/column";
import DataTable from "primevue/datatable";
import ProgressBar from "primevue/progressbar";

import { formatBytes, formatEta, formatSpeed, progressLabel, progressValue, stateLabel } from "../../lib/download-format";
import type { DownloadSummary } from "../../types/download";

const props = defineProps<{
  downloads: DownloadSummary[];
  errorMessage: string;
  infoMessage: string;
  isAutoRefreshing: boolean;
  isRefreshingList: boolean;
  selectedId: string | null;
}>();

defineEmits<{
  refresh: [];
  select: [downloadId: string];
}>();
</script>

<template>
  <section class="queue-panel desk-panel">
    <div class="desk-panel__header">
      <div>
        <h2 class="panel-title">任务列表</h2>
      </div>

      <div class="queue-panel__actions">
        <Button type="button" size="small" severity="secondary" @click="$emit('refresh')">
          {{ isRefreshingList ? "刷新中…" : "刷新" }}
        </Button>
      </div>
    </div>

    <p v-if="infoMessage" class="status-banner status-banner--info">{{ infoMessage }}</p>
    <p v-if="errorMessage" class="status-banner status-banner--error">{{ errorMessage }}</p>

    <div class="queue-panel__table">
      <DataTable
        :value="downloads"
        data-key="id"
        scrollable
        scroll-height="flex"
        size="small"
        class="queue-table"
        :rowClass="(data) => data.id === selectedId ? 'queue-row--active' : ''"
        @row-click="(e) => $emit('select', e.data.id)"
      >
        <Column field="fileName" header="文件名" style="min-width: 15rem" />
        <Column field="destinationPath" header="保存路径" style="min-width: 15rem" />

        <Column header="状态">
          <template #body="{ data }">
            <span class="state-pill" :data-state="data.state">{{ stateLabel(data.state) }}</span>
          </template>
        </Column>

        <Column header="进度">
          <template #body="{ data }">
            <div class="queue-progress">
              <div class="queue-progress__copy">
                <span>{{ progressLabel(data) }}</span>
                <span>{{ formatBytes(data.downloadedBytes) }} / {{ formatBytes(data.totalBytes) }}</span>
              </div>
              <ProgressBar :value="progressValue(data)" :show-value="false" />
            </div>
          </template>
        </Column>

        <Column header="速度">
          <template #body="{ data }">
            <span class="queue-meta">{{ formatSpeed(data.speedBytesPerSecond) }}</span>
          </template>
        </Column>

        <Column header="剩余时间">
          <template #body="{ data }">
            <span class="queue-meta">{{ formatEta(data.etaSeconds) }}</span>
          </template>
        </Column>



        <template #empty>
          <div class="queue-empty">
            <h3>暂无下载任务</h3>
            <p>点击左侧“新建任务”开始下载。</p>
          </div>
        </template>
      </DataTable>
    </div>
  </section>
</template>

<style scoped>
.queue-panel {
  display: grid;
  gap: var(--space-4);
  padding: var(--space-5);
}

.queue-panel__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.sync-pill {
  display: inline-flex;
  align-items: center;
  min-height: var(--control-height-compact);
  padding-inline: var(--space-3);
  border-radius: var(--radius-round);
  border: var(--border-width-thin) solid var(--color-border-strong);
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
  font-size: var(--font-size-label);
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

.sync-pill[data-active="true"] {
  color: var(--color-accent-strong);
  border-color: var(--color-accent-border);
  background: var(--color-accent-soft);
}

.queue-panel__table {
  min-height: 0;
  height: var(--queue-height);
}

.queue-panel__table :deep(.queue-row--active) {
  background-color: var(--color-accent-soft) !important;
  color: var(--color-accent-strong) !important;
}

.queue-panel__table :deep(.queue-row--active) td {
  font-weight: 600;
}

.queue-progress {
  display: grid;
  gap: var(--space-2);
}

.queue-progress__copy {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

.queue-empty {
  display: grid;
  gap: var(--space-2);
  place-items: center;
  min-height: var(--queue-height);
  text-align: center;
  color: var(--color-text-muted);
}

.queue-empty h3,
.queue-empty p {
  margin: 0;
}
</style>
