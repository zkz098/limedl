<script setup lang="ts">
import { computed, ref, watch } from "vue";

import BtPeerTable from "./BtPeerTable.vue";
import BtTrackerTable from "./BtTrackerTable.vue";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import { useI18n } from "../../i18n";
import { toFriendlyError } from "../../composables/downloadHelpers";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  isSizeUnknown,
  progressValue,
} from "../../lib/download-format";
import {
  getBtFiles,
  getBtPeers,
  getBtTrackers,
  getBtPieces,
  updateBtFiles,
} from "../../lib/tauri/download-api";
import type {
  BtFileStatus,
  BtPeerInfo,
  BtPieceInfo,
  BtTrackerInfo,
  DownloadSnapshot,
  DownloadSummary,
} from "../../types/download";

const props = defineProps<{
  selectedOverview: DownloadSummary | DownloadSnapshot;
  selectedSnapshot: DownloadSnapshot | null;
  showDetailInfo: boolean;
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
  mono?: boolean;
}

const detailRows = computed<DetailRow[]>(() => {
  const snapshot = props.selectedSnapshot;

  if (!snapshot) {
    return [];
  }

  const commonRows = [
    { label: t("inspector.fields.url"), value: snapshot.url, wide: true, mono: true },
    { label: t("inspector.fields.destinationPath"), value: snapshot.destinationPath, wide: true, mono: true },
  ];

  if (snapshot.kind === "bt") {
    return [
      ...commonRows,
      {
        label: t("inspector.fields.infoHash"),
        value: snapshot.infoHash ?? t("common.dash"),
        wide: true,
        mono: true,
      },
    ];
  }

  return [
    ...commonRows,
    {
      label: t("inspector.fields.checksum"),
      value: snapshot.checksum ?? t("common.dash"),
      wide: true,
    },
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

// ── BT file list ──

const btFileList = ref<BtFileStatus[]>([]);
const isFetchingBtFiles = ref(false);
const isUpdatingFiles = ref(false);

async function fetchBtFiles(downloadId: string) {
  isFetchingBtFiles.value = true;
  try {
    btFileList.value = await getBtFiles(downloadId);
  } catch {
    btFileList.value = [];
  } finally {
    isFetchingBtFiles.value = false;
  }
}

async function toggleFileInclusion(fileIndex: number, currentlyIncluded: boolean) {
  const newIncluded = new Set(btFileList.value.filter((f) => f.included).map((f) => f.index));
  if (currentlyIncluded) {
    newIncluded.delete(fileIndex);
  } else {
    newIncluded.add(fileIndex);
  }
  // Prevent deselecting all files — at least one must remain
  if (newIncluded.size === 0) {
    return;
  }
  isUpdatingFiles.value = true;
  try {
    await updateBtFiles(props.selectedSnapshot!.id, [...newIncluded]);
    // Optimistic local update
    const file = btFileList.value.find((f) => f.index === fileIndex);
    if (file) file.included = !currentlyIncluded;
  } catch {
    // Revert on error — refetch
    if (props.selectedSnapshot?.id) {
      await fetchBtFiles(props.selectedSnapshot.id);
    }
  } finally {
    isUpdatingFiles.value = false;
  }
}

function clearBtData() {
  peerList.value = [];
  trackerList.value = [];
  pieceList.value = [];
  btFileList.value = [];
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

// Fetch BT file list whenever the Files tab becomes active
watch([() => props.selectedSnapshot?.id, activeTab], ([id, tab]) => {
  if (id && props.selectedSnapshot?.kind === "bt" && tab === "files") {
    void fetchBtFiles(id);
  }
});
</script>

<template>
  <section class="download-inspector">
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
              </div>
              <p>{{ selectedOverview.destinationPath }}</p>
            </div>

            <div class="metric-grid">
              <div class="metric-card">
                <span class="metric-card__label">{{ t("inspector.transferred") }}</span>
                <span class="metric-card__value">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
              <div class="metric-card">
                <span class="metric-card__label">{{ t("inspector.speed") }}</span>
                <span class="metric-card__value">{{
                  formatSpeed(selectedOverview.speedBytesPerSecond)
                }}</span>
              </div>
              <div class="metric-card">
                <span class="metric-card__label">{{ t("inspector.eta") }}</span>
                <span class="metric-card__value">{{ formatEta(selectedOverview.etaSeconds) }}</span>
              </div>
              <div class="metric-card">
                <span class="metric-card__label">{{
                  selectedOverview.kind === "bt" ? t("inspector.peers") : t("inspector.threads")
                }}</span>
                <span class="metric-card__value">
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
              <div v-if="selectedOverview.kind === 'bt'" class="metric-card">
                <span class="metric-card__label">{{ t("inspector.fields.seedCount") }}</span>
                <span class="metric-card__value"
                  >{{ selectedOverview.seedCount ?? 0 }} /
                  {{ selectedOverview.leechCount ?? 0 }}</span
                >
              </div>
              <div v-if="selectedOverview.cdnAccelerated" class="metric-card">
                <span class="metric-card__label">{{ t("inspector.cdnNode") }}</span>
                <span class="metric-card__value">
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
                :indeterminate="
                  isSizeUnknown(selectedOverview) && selectedOverview.state !== 'completed'
                "
              />
            </div>
          </div>

          <div
            v-if="selectedSnapshot?.kind === 'http' && selectedSnapshot.chunks?.length"
            class="chunk-progress-text"
          >
            {{
              t("inspector.chunkProgressText", {
                completed: selectedSnapshot.chunks.filter((c) => c.completed).length,
                total: selectedSnapshot.chunks.length,
              })
            }}
          </div>

          <dl v-if="showDetailInfo && detailRows.length" class="detail-grid">
            <div
              v-for="row in detailRows"
              :key="row.label"
              class="text-row"
              :class="{ 'text-row--wide': row.wide }"
            >
              <dt class="text-label">{{ row.label }}:</dt>
              <dd class="text-value" :class="{ 'text-value--mono': row.mono }">{{ row.value }}</dd>
            </div>
          </dl>

          <div v-if="selectedOverview.error" class="status-banner status-banner--error">
            <span class="status-banner__icon i-ri-error-warning-line" aria-hidden="true" />
            <span class="status-banner__message">{{ toFriendlyError(selectedOverview.error) }}</span>
          </div>
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
                <span class="text-value text-value--mono">{{ selectedOverview.destinationPath }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.url") }}:</span>
                <span class="text-value text-value--mono">{{ selectedOverview.url }}</span>
              </div>
              <div v-if="selectedSnapshot?.finalUrl" class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.fields.finalUrl") }}:</span>
                <span class="text-value text-value--mono">{{ selectedSnapshot.finalUrl }}</span>
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
                <span class="text-value text-value--mono">{{ selectedOverview.destinationPath }}</span>
              </div>
              <div v-if="selectedOverview.infoHash" class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.fields.infoHash") }}:</span>
                <span class="text-value text-value--mono">{{ selectedOverview.infoHash }}</span>
              </div>
              <div class="text-row text-row--wide">
                <span class="text-label">{{ t("inspector.files.url") }}:</span>
                <span class="text-value text-value--mono">{{ selectedOverview.url }}</span>
              </div>
              <div class="text-row">
                <span class="text-label">{{ t("inspector.transferred") }}:</span>
                <span class="text-value">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
            </div>

            <section class="inspector-bt-section">
              <div class="inspector-section-header">
                <h3>{{ t("inspector.files.fileCount", { count: btFileList.length }) }}</h3>
                <UiButton
                  variant="ghost"
                  size="sm"
                  icon="i-ri-refresh-line"
                  :loading="isFetchingBtFiles"
                  @click="fetchBtFiles(selectedSnapshot!.id)"
                >
                  {{ t("inspector.files.refreshFiles") }}
                </UiButton>
              </div>

              <div
                v-if="isFetchingBtFiles && btFileList.length === 0"
                class="file-list-placeholder"
              >
                {{ t("inspector.files.loadingFiles") }}
              </div>

              <div v-else-if="btFileList.length === 0" class="file-list-placeholder">
                {{ t("inspector.files.noFileList") }}
              </div>

              <div v-else class="bt-file-list">
                <div
                  v-for="file in btFileList"
                  :key="file.index"
                  class="bt-file-row"
                  :class="{ 'bt-file-row--excluded': !file.included }"
                >
                  <label class="bt-file-checkbox" @click.stop>
                    <input
                      type="checkbox"
                      :checked="file.included"
                      :disabled="isUpdatingFiles"
                      @change="toggleFileInclusion(file.index, file.included)"
                    />
                  </label>
                  <div class="bt-file-info">
                    <span class="bt-file-path">{{ file.path }}</span>
                    <div class="bt-file-meta">
                      <span>{{ formatBytes(file.size) }}</span>
                      <span class="bt-file-separator">·</span>
                      <span>{{ formatBytes(file.downloadedBytes) }}</span>
                      <span class="bt-file-separator">·</span>
                      <span v-if="file.included" class="bt-file-included">{{
                        t("inspector.files.included")
                      }}</span>
                      <span v-else class="bt-file-excluded-label">{{
                        t("inspector.files.excluded")
                      }}</span>
                    </div>
                    <UiProgress
                      :value="file.size > 0 ? (file.downloadedBytes / file.size) * 100 : 0"
                      class="bt-file-progress"
                    />
                  </div>
                </div>
              </div>
            </section>
          </template>
        </div>

        <!-- ── Peers & Trackers tab (BT only) ── -->
        <div v-show="activeTab === 'peersTrackers'" v-if="isBtTask" class="inspector-tab-content">
          <div v-if="pieceList.length" class="piece-progress-text">
            {{
              t("inspector.pieceProgressText", {
                completed: pieceList.filter((p) => p.completed).length,
                total: pieceList.length,
              })
            }}
          </div>

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
  </section>
</template>

<style scoped>
.download-inspector {
  display: flex;
  flex-direction: column;
  height: 100%;
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

.inspector-summary__copy h3,
.inspector-summary__copy p {
  margin: 0;
}

.inspector-summary__copy p {
  color: var(--color-text-muted);
  font-size: 0.8rem;
  font-family: var(--font-mono);
}

.inspector-summary__copy h3 {
  color: var(--color-heading);
  font-size: var(--font-size-body);
}

/* ── Metric grid ── */

.metric-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.55rem;
}

.metric-card {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  padding: 0.6rem 0.7rem;
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease;
}

.metric-card:hover {
  background: var(--color-surface-muted);
  border-color: var(--color-border-strong);
}

.metric-card__label {
  color: var(--color-text-muted);
  font-size: 0.7rem;
  font-weight: 500;
  letter-spacing: var(--letter-spacing-wide);
  text-transform: uppercase;
}

.metric-card__value {
  color: var(--color-text-main);
  font-size: var(--font-size-metric);
  font-weight: var(--font-weight-semibold);
  line-height: 1.35;
  word-break: break-all;
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
}

.text-value--mono {
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
}

.chunk-progress-text,
.piece-progress-text {
  color: var(--color-text-muted);
  font-size: 0.72rem;
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

/* ── BT file list ── */

.bt-file-list {
  display: grid;
  gap: 0.35rem;
  max-height: 360px;
  overflow-y: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--space-1);
  background: var(--color-panel-muted);
}

.bt-file-row {
  display: flex;
  gap: var(--space-2);
  align-items: flex-start;
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  transition: background 0.15s;
}

.bt-file-row:hover {
  background: var(--color-surface-muted);
}

.bt-file-row--excluded {
  opacity: 0.55;
}

.bt-file-checkbox {
  display: flex;
  align-items: center;
  padding-top: 0.15rem;
  cursor: pointer;
}

.bt-file-checkbox input[type="checkbox"] {
  accent-color: var(--color-accent);
  cursor: pointer;
}

.bt-file-info {
  flex: 1;
  min-width: 0;
  display: grid;
  gap: 0.2rem;
}

.bt-file-path {
  font-size: 0.78rem;
  font-family: var(--font-mono);
  color: var(--color-text-main);
  word-break: break-all;
  line-height: 1.35;
}

.bt-file-meta {
  font-size: 0.68rem;
  color: var(--color-text-muted);
  display: flex;
  gap: 0.3rem;
  align-items: center;
  flex-wrap: wrap;
}

.bt-file-separator {
  color: var(--color-border-strong);
}

.bt-file-included {
  color: var(--color-success);
}

.bt-file-excluded-label {
  color: var(--color-text-muted);
}

.bt-file-progress {
  max-width: 100%;
}

/* ── Status banner ── */

.status-banner {
  margin: 0;
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
}

.status-banner--error {
  border-left: 3px solid var(--color-danger-text);
  box-shadow: var(--shadow-card);
}

.status-banner__icon {
  flex-shrink: 0;
  margin-top: 0.15rem;
  font-size: 1.1rem;
  line-height: 1;
}

.status-banner__message {
  flex: 1 1 auto;
  line-height: 1.6;
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
