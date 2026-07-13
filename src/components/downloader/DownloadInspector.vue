<script setup lang="ts">
import { computed, ref, watch } from "vue";

import BtPeerTable from "./BtPeerTable.vue";
import BtPieceHeatmap from "./BtPieceHeatmap.vue";
import BtTrackerTable from "./BtTrackerTable.vue";
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
  isSizeUnknown,
  progressValue,
} from "../../lib/download-format";
import { getBtPeers, getBtTrackers, getBtPieces } from "../../lib/tauri/download-api";
import type {
  BtPeerInfo,
  BtPieceInfo,
  BtTrackerInfo,
  DownloadSnapshot,
  DownloadSummary,
} from "../../types/download";

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

// ── Tab state ──

const activeTab = ref<"overview" | "files" | "peersTrackers">("overview");

const isBtTask = computed(() => props.selectedOverview?.kind === "bt");

watch(
  () => props.selectedOverview?.id,
  () => {
    activeTab.value = "overview";
  },
);

watch(isBtTask, (bt) => {
  if (!bt && activeTab.value === "peersTrackers") {
    activeTab.value = "overview";
  }
});

// ── Detail rows ──

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
      { label: t("inspector.fields.seedCount"), value: snapshot.seedCount?.toString() ?? t("common.dash") },
      { label: t("inspector.fields.leechCount"), value: snapshot.leechCount?.toString() ?? t("common.dash") },
      { label: t("inspector.fields.peerCount"), value: String(snapshot.peerCount ?? 0) },
      { label: t("inspector.fields.uploadedBytes"), value: formatBytes(snapshot.uploadedBytes) },
      { label: t("inspector.fields.downloadLimitBps"), value: formatSpeed(snapshot.downloadLimitBps) },
      { label: t("inspector.fields.uploadLimitBps"), value: formatSpeed(snapshot.uploadLimitBps) },
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

// ── BT on-demand data ──

const peerList = ref<BtPeerInfo[]>([]);
const trackerList = ref<BtTrackerInfo[]>([]);
const pieceList = ref<BtPieceInfo[]>([]);
const isFetchingPeers = ref(false);
const isFetchingTrackers = ref(false);
const isFetchingPieces = ref(false);
const lastBtFetchAt = ref(0);

function clearBtData() {
  peerList.value = [];
  trackerList.value = [];
  pieceList.value = [];
}

async function fetchBtPeers(downloadId: string) {
  isFetchingPeers.value = true;
  try {
    peerList.value = await getBtPeers(downloadId);
  } catch {
    peerList.value = [];
  } finally {
    isFetchingPeers.value = false;
  }
}

async function fetchBtTrackers(downloadId: string) {
  isFetchingTrackers.value = true;
  try {
    trackerList.value = await getBtTrackers(downloadId);
  } catch {
    trackerList.value = [];
  } finally {
    isFetchingTrackers.value = false;
  }
}

async function fetchBtPieces(downloadId: string) {
  isFetchingPieces.value = true;
  try {
    pieceList.value = await getBtPieces(downloadId);
  } catch {
    pieceList.value = [];
  } finally {
    isFetchingPieces.value = false;
  }
}

watch(
  () => props.selectedSnapshot?.id,
  (id) => {
    if (id && props.selectedSnapshot?.kind === "bt") {
      const now = Date.now();
      if (now - lastBtFetchAt.value < 5000) return;
      lastBtFetchAt.value = now;
      void Promise.all([fetchBtPeers(id), fetchBtTrackers(id), fetchBtPieces(id)]);
    } else {
      clearBtData();
    }
  },
  { immediate: true },
);
</script>

<template>
  <section class="download-inspector">
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

    <template v-if="selectedOverview">
      <!-- Tab bar -->
      <div class="inspector-tabs">
        <button
          class="inspector-tab"
          :class="{ active: activeTab === 'overview' }"
          @click="activeTab = 'overview'"
        >
          {{ t("inspector.tabs.overview") }}
        </button>
        <button
          class="inspector-tab"
          :class="{ active: activeTab === 'files' }"
          @click="activeTab = 'files'"
        >
          {{ t("inspector.tabs.files") }}
        </button>
        <button
          v-if="isBtTask"
          class="inspector-tab"
          :class="{ active: activeTab === 'peersTrackers' }"
          @click="activeTab = 'peersTrackers'"
        >
          {{ t("inspector.tabs.peersTrackers") }}
        </button>
      </div>

      <!-- Tab content -->
      <div class="inspector-content">
        <!-- ── Overview tab ── -->
        <div v-show="activeTab === 'overview'" class="inspector-tab-content">
          <div class="inspector-summary">
            <div class="inspector-summary__copy">
              <div class="inspector-summary__header">
                <h3>{{ selectedOverview.fileName }}</h3>
                <UiBadge :tone="stateTone">{{ t(`states.${selectedOverview.state}`) }}</UiBadge>
                <UiBadge
                  v-if="selectedOverview.cdnAccelerated"
                  tone="warning"
                  size="sm"
                  class="inspector-summary__cdn"
                >
                  <span class="i-ri-flashlight-fill" aria-hidden="true" />
                  {{ t("inspector.cdnAccelerated") }}
                </UiBadge>
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
              <div
                v-if="selectedOverview.kind === 'bt'"
                class="text-row"
              >
                <span class="text-label">{{ t("inspector.fields.seedCount") }}:</span>
                <span class="text-value">{{ selectedOverview.seedCount ?? 0 }} / {{ selectedOverview.leechCount ?? 0 }}</span>
              </div>
              <div v-if="selectedOverview.cdnAccelerated" class="text-row">
                <span class="text-label">{{ t("inspector.cdnNode") }}:</span>
                <span class="text-value">
                  <span class="i-ri-flashlight-fill" aria-hidden="true" />
                  {{ t("inspector.cdnAccelerated") }}
                </span>
              </div>
            </div>

            <div class="summary-progress">
              <div class="summary-progress__copy">
                <span>{{ t("inspector.progress") }}</span>
                <span>{{ progressValue(selectedOverview).toFixed(1) }}%</span>
              </div>
              <UiProgress
              :value="progressValue(selectedOverview)"
              :indeterminate="isSizeUnknown(selectedOverview) && selectedOverview.state !== 'completed'"
            />
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

        <!-- ── Files tab ── -->
        <div v-show="activeTab === 'files'" class="inspector-tab-content">
          <template v-if="selectedOverview.kind === 'http'">
            <div class="detail-grid">
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.filename") }}:</span>
                <span class="text-value">{{ selectedOverview.fileName }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.destination") }}:</span>
                <span class="text-value">{{ selectedOverview.destinationPath }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.url") }}:</span>
                <span class="text-value">{{ selectedOverview.url }}</span>
              </div>
              <div v-if="selectedSnapshot?.finalUrl" class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.fields.finalUrl") }}:</span>
                <span class="text-value">{{ selectedSnapshot.finalUrl }}</span>
              </div>
              <div class="text-row">
                <span class="text-label">{{ t("inspector.transferred") }}:</span>
                <span class="text-value">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
            </div>
          </template>
          <template v-else>
            <div class="detail-grid">
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.filename") }}:</span>
                <span class="text-value">{{ selectedOverview.fileName }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.destination") }}:</span>
                <span class="text-value">{{ selectedOverview.destinationPath }}</span>
              </div>
              <div v-if="selectedOverview.infoHash" class="text-row text-row--wide">
                <span class="text-label">{{ t('inspector.fields.infoHash') }}:</span>
                <span class="text-value" style="font-family: var(--font-mono); word-break: break-all;">{{ selectedOverview.infoHash }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.url") }}:</span>
                <span class="text-value">{{ selectedOverview.url }}</span>
              </div>
              <div class="text-row">
                <span class="text-label">{{ t("inspector.transferred") }}:</span>
                <span class="text-value">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
            </div>
            <p class="file-list-placeholder">{{ t("inspector.files.noFileList") }}</p>
          </template>
        </div>

        <!-- ── Peers & Trackers tab (BT only) ── -->
        <div v-show="activeTab === 'peersTrackers'" v-if="isBtTask" class="inspector-tab-content">
          <BtPieceHeatmap
            v-if="showHeatmap && selectedSnapshot?.kind === 'bt'"
            :pieces="pieceList"
            :title="t('inspector.sections.pieces')"
          />

          <section class="inspector-bt-section">
            <div class="inspector-section-header">
              <h3>{{ t("inspector.sections.peers") }} ({{ peerList.length }})</h3>
              <UiButton
                variant="ghost"
                size="sm"
                icon="i-ri-refresh-line"
                :loading="isFetchingPeers"
                @click="fetchBtPeers(selectedSnapshot!.id)"
              >
                {{ t("inspector.refreshPeers") }}
              </UiButton>
            </div>
            <BtPeerTable :peers="peerList" />
          </section>

          <section class="inspector-bt-section">
            <div class="inspector-section-header">
              <h3>{{ t("inspector.sections.trackers") }} ({{ trackerList.length }})</h3>
              <UiButton
                variant="ghost"
                size="sm"
                icon="i-ri-refresh-line"
                :loading="isFetchingTrackers"
                @click="fetchBtTrackers(selectedSnapshot!.id)"
              >
                {{ t("inspector.refreshTrackers") }}
              </UiButton>
            </div>
            <BtTrackerTable :trackers="trackerList" />
          </section>
        </div>
      </div>
    </template>

    <div v-else class="inspector-empty">
      <h3>{{ t("inspector.noSelectionTitle") }}</h3>
      <p>{{ t("inspector.noSelectionDescription") }}</p>
    </div>
  </section>
</template>

<style scoped>
.download-inspector {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.download-inspector :deep(.section-kicker) {
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
  font-size: 0.7rem;
}

.inspector-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
  padding: var(--space-3) var(--space-4);
  flex-shrink: 0;
}

.inspector-actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

/* ── Tab bar ── */

.inspector-tabs {
  display: flex;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3) 0;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.inspector-tab {
  padding: var(--space-1) var(--space-3);
  border: none;
  background: none;
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--color-text-muted);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
}

.inspector-tab:hover {
  color: var(--color-text-main);
  background: var(--color-surface-muted);
}

.inspector-tab.active {
  color: var(--color-accent-strong);
  border-bottom-color: var(--color-accent);
}

/* ── Content area ── */

.inspector-content {
  flex: 1;
  overflow: auto;
  min-height: 0;
  padding: var(--space-3);
}

.inspector-tab-content {
  display: grid;
  gap: 0.75rem;
}

/* ── Summary ── */

.inspector-summary {
  display: grid;
  gap: 0.65rem;
}

.inspector-summary__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.inspector-summary__cdn {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
}

.inspector-summary__cdn .i-ri-flashlight-fill {
  font-size: 0.8rem;
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
  font-size: 0.8rem;
  font-family: var(--font-mono);
}

.inspector-summary__copy h3,
.inspector-empty h3 {
  color: var(--color-heading);
  font-size: var(--font-size-body);
}

/* ── Metric grid ── */

.metric-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.35rem 0.9rem;
  padding: 0.65rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
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
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

.text-value {
  color: var(--color-text-main);
  word-break: break-all;
  font-size: 0.8rem;
  line-height: 1.45;
  font-family: var(--font-mono);
}

/* ── Progress ── */

.summary-progress {
  display: grid;
  gap: 0.3rem;
}

.summary-progress__copy {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--color-text-muted);
  font-size: 0.72rem;
  font-family: var(--font-mono);
}

/* ── Detail grid ── */

.detail-grid {
  margin: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.35rem 0.9rem;
  padding: 0.65rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
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

/* ── BT sections ── */

.inspector-bt-section {
  display: grid;
  gap: 0.65rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.inspector-section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-2);
}

.inspector-section-header h3 {
  margin: 0;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--color-text-muted);
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

/* ── Empty state ── */

.inspector-empty {
  display: grid;
  gap: var(--space-2);
  min-height: 14rem;
  place-content: center;
  text-align: center;
  padding: var(--space-3);
}

/* ── File list placeholder ── */

.file-list-placeholder {
  margin: 0;
  padding: 1rem;
  text-align: center;
  color: var(--color-text-muted);
  font-size: 0.8rem;
  border: 1px dashed var(--color-border-strong);
  border-radius: var(--radius-md);
}

/* ── Status banner ── */

.status-banner {
  margin: 0;
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
