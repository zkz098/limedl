<script setup lang="ts">
import { computed } from "vue";

import ChunkHeatmap from "./ChunkHeatmap.vue";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import { useI18n } from "../../i18n";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  formatTimestamp,
  progressValue,
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
  showDetailInfo: boolean;
  showHeatmap: boolean;
}>();

defineEmits<{
  cancel: [];
  pause: [];
  refresh: [];
  resume: [];
  close: [];
}>();

const { t } = useI18n();

interface DetailRow {
  label: string;
  value: string;
  wide?: boolean;
}

const detailRows = computed<DetailRow[]>(() => {
  const snapshot = props.selectedSnapshot;

  if (!snapshot) {
    return [];
  }

  const commonRows = [
    { label: t("inspector.fields.url"), value: snapshot.url, wide: true },
    { label: t("inspector.fields.destinationPath"), value: snapshot.destinationPath, wide: true },
  ];

  if (snapshot.kind === "bt") {
    return [
      ...commonRows,
      {
        label: t("inspector.fields.uploadStatus"),
        value: snapshot.uploadStatus
          ? t(`uploadStatus.${snapshot.uploadStatus}`)
          : t("uploadStatus.idle"),
      },
      { label: t("inspector.fields.peerCount"), value: String(snapshot.peerCount ?? 0) },
      { label: t("inspector.fields.uploadedBytes"), value: formatBytes(snapshot.uploadedBytes) },
      { label: t("inspector.fields.createdAt"), value: formatTimestamp(snapshot.createdAtMs) },
      { label: t("inspector.fields.updatedAt"), value: formatTimestamp(snapshot.updatedAtMs) },
    ];
  }

  return [
    ...commonRows,
    { label: t("inspector.fields.finalUrl"), value: snapshot.finalUrl, wide: true },
    { label: t("inspector.fields.tempPath"), value: snapshot.tempPath, wide: true },
    {
      label: t("inspector.fields.supportsRanges"),
      value: snapshot.supportsRanges ? t("common.supported") : t("common.unsupported"),
    },
    { label: t("inspector.fields.connectionCount"), value: String(snapshot.connectionCount) },
    { label: t("inspector.fields.threadMode"), value: t(`tokens.${snapshot.threadMode}`) },
    {
      label: t("inspector.fields.requestedThreadCount"),
      value: snapshot.requestedThreadCount
        ? String(snapshot.requestedThreadCount)
        : t("common.dash"),
    },
    {
      label: t("inspector.fields.desiredThreadCount"),
      value: snapshot.desiredThreadCount ? String(snapshot.desiredThreadCount) : t("common.dash"),
    },
    {
      label: t("inspector.fields.allocatedThreadCount"),
      value: snapshot.allocatedThreadCount
        ? String(snapshot.allocatedThreadCount)
        : t("common.dash"),
    },
    {
      label: t("inspector.fields.adaptiveProfile"),
      value: snapshot.adaptiveProfile ? t(`tokens.${snapshot.adaptiveProfile}`) : t("common.dash"),
    },
    {
      label: t("inspector.fields.threadNote"),
      value: snapshot.threadNote ?? t("common.dash"),
      wide: true,
    },
    { label: t("inspector.fields.checksumMode"), value: t(`tokens.${snapshot.checksumMode}`) },
    { label: t("inspector.fields.checksum"), value: snapshot.checksum ?? t("common.dash") },
    { label: t("inspector.fields.etag"), value: snapshot.etag ?? t("common.dash") },
    { label: t("inspector.fields.lastModified"), value: snapshot.lastModified ?? t("common.dash") },
    { label: t("inspector.fields.createdAt"), value: formatTimestamp(snapshot.createdAtMs) },
    { label: t("inspector.fields.updatedAt"), value: formatTimestamp(snapshot.updatedAtMs) },
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
        <p class="section-kicker">{{ t("inspector.kicker") }}</p>
        <h2 class="panel-title">{{ t("inspector.title") }}</h2>
      </div>
      <div class="inspector-actions">
        <UiButton
          type="button"
          size="sm"
          variant="secondary"
          icon="i-ri-refresh-line"
          @click="$emit('refresh')"
        >
          {{ isRefreshingStatus ? t("common.refreshing") : t("common.refresh") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-pause-line"
          :disabled="!canPause"
          @click="$emit('pause')"
        >
          {{ actionName === "Pause" ? t("inspector.pausing") : t("inspector.pause") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="ghost"
          icon="i-ri-play-line"
          :disabled="!canResume"
          @click="$emit('resume')"
        >
          {{ actionName === "Resume" ? t("inspector.resuming") : t("inspector.resume") }}
        </UiButton>
        <UiButton
          type="button"
          size="sm"
          variant="danger"
          icon="i-ri-close-circle-line"
          :disabled="!canCancel"
          @click="$emit('cancel')"
        >
          {{ actionName === "Cancel" ? t("inspector.canceling") : t("inspector.cancel") }}
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
            <UiBadge :tone="stateTone">{{ t(`states.${selectedOverview.state}`) }}</UiBadge>
          </div>
          <p>{{ selectedOverview.destinationPath }}</p>
        </div>

        <div class="metric-grid">
          <div class="text-row">
            <span class="text-label">{{ t("inspector.transferred") }}:</span>
            <span class="text-value">
              {{ formatBytes(selectedOverview.downloadedBytes) }} /
              {{ formatBytes(selectedOverview.totalBytes) }}
            </span>
          </div>
          <div class="text-row">
            <span class="text-label">{{ t("inspector.speed") }}:</span>
            <span class="text-value">{{ formatSpeed(selectedOverview.speedBytesPerSecond) }}</span>
          </div>
          <div class="text-row">
            <span class="text-label">{{ t("inspector.eta") }}:</span>
            <span class="text-value">{{ formatEta(selectedOverview.etaSeconds) }}</span>
          </div>
          <div class="text-row">
            <span class="text-label"
              >{{
                selectedOverview.kind === "bt" ? t("inspector.peers") : t("inspector.threads")
              }}:</span
            >
            <span class="text-value">
              {{
                selectedOverview.kind === "bt"
                  ? (selectedOverview.peerCount ?? 0)
                  : selectedOverview.connectionCount
              }}
              <template
                v-if="
                  selectedOverview.kind === 'http' && selectedOverview.threadMode === 'adaptive'
                "
              >
                / {{ t("tokens.adaptive") }}
              </template>
            </span>
          </div>
        </div>

        <div class="summary-progress">
          <div class="summary-progress__copy">
            <span>{{ t("inspector.progress") }}</span>
            <span>{{ progressValue(selectedOverview).toFixed(1) }}%</span>
          </div>
          <UiProgress :value="progressValue(selectedOverview)" />
        </div>
      </div>

      <ChunkHeatmap
        v-if="
          showHeatmap
          && selectedSnapshot?.kind === 'http'
          && selectedSnapshot.supportsRanges
          && selectedSnapshot.chunks?.length
        "
        :chunks="selectedSnapshot.chunks"
        :title="t('inspector.chunkProgress')"
        :totalBytes="selectedSnapshot.totalBytes ?? 0"
      />

      <dl v-if="showDetailInfo && detailRows.length" class="detail-grid">
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
      <h3>{{ t("inspector.noSelectionTitle") }}</h3>
      <p>{{ t("inspector.noSelectionDescription") }}</p>
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
