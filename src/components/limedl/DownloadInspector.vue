<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";

import BtPeerTable from "./BtPeerTable.vue";
import BtTrackerTable from "./BtTrackerTable.vue";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import { useI18n } from "../../i18n";
import { toFriendlyError, toneForState } from "../../composables/downloadHelpers";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  isSizeUnknown,
  progressValue,
} from "../../lib/download-format";
import { useBtInspector } from "../../composables/useBtInspector";
import type { DownloadSnapshot, DownloadSummary } from "../../types/download";

const props = defineProps<{
  selectedOverview: DownloadSummary | DownloadSnapshot;
  selectedSnapshot: DownloadSnapshot | null;
  showDetailInfo: boolean;
}>();

const { t } = useI18n();

// ── Tab state ──

const activeTab = ref<"overview" | "files" | "peersTrackers">("overview");

const isBtTask = computed(() => props.selectedOverview?.kind === "bt");

// Use selectedOverview (summary + snapshot) instead of selectedSnapshot alone,
// because selectedSnapshot may be null when the inspector first opens (e.g. after
// a page refresh, the snapshot is fetched separately). The summary is available
// immediately and carries the same id+kind needed for BT queries.
const btTaskId = computed(() =>
  props.selectedOverview?.kind === "bt" ? props.selectedOverview.id : null,
);

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
    {
      label: t("inspector.fields.destinationPath"),
      value: snapshot.destinationPath,
      wide: true,
      mono: true,
    },
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

const stateTone = computed(() => toneForState(props.selectedOverview?.state ?? ""));

// ── BT composable ──

const {
  files: btFileList,
  peers: peerList,
  trackers: trackerList,
  pieces: pieceList,
  isLoading,
  errors: btErrors,
  isUpdatingFiles,
  fetchFiles: fetchBtFiles,
  fetchPeers: fetchBtPeers,
  fetchTrackers: fetchBtTrackers,
  fetchPieces: fetchBtPieces,
  toggleFileInclusion,
} = useBtInspector(btTaskId);

const isFetchingBtFiles = computed(() => isLoading.files);
const isFetchingPeers = computed(() => isLoading.peers);
const isFetchingTrackers = computed(() => isLoading.trackers);

// Fetch BT file list whenever the Files tab becomes active
watch([btTaskId, activeTab], ([id, tab]) => {
  if (id && tab === "files") {
    void fetchBtFiles();
  }
});

// Fetch BT peers/trackers/pieces whenever the Peers & Trackers tab becomes active
watch([btTaskId, activeTab], ([id, tab]) => {
  if (id && tab === "peersTrackers") {
    void Promise.all([fetchBtPeers(), fetchBtTrackers(), fetchBtPieces()]);
  }
});

// Poll peers/trackers data every 5s while the tab is active
let peersTrackerInterval: ReturnType<typeof setInterval> | null = null;
watch(
  [btTaskId, activeTab],
  ([id, tab]) => {
    if (peersTrackerInterval) {
      clearInterval(peersTrackerInterval);
      peersTrackerInterval = null;
    }
    if (id && tab === "peersTrackers") {
      peersTrackerInterval = setInterval(() => {
        void Promise.all([fetchBtPeers(), fetchBtTrackers(), fetchBtPieces()]);
      }, 5000);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (peersTrackerInterval) {
    clearInterval(peersTrackerInterval);
    peersTrackerInterval = null;
  }
});
</script>

<template>
  <section class="flex flex-col h-full">
    <template v-if="selectedOverview">
      <!-- Tab bar -->
      <div
        class="flex gap-[var(--space-1)] px-[var(--space-3)] pt-[var(--space-2)] border-b border-[var(--color-border)] shrink-0"
      >
        <button
          type="button"
          class="px-[var(--space-3)] py-[var(--space-1)] border-none bg-transparent text-[0.8125rem] font-medium text-[var(--color-text-muted)] cursor-pointer border-b-2 border-transparent -mb-px rounded-t-[var(--radius-sm)] transition-colors duration-150 hover:text-[var(--color-text-main)] hover:bg-[var(--color-surface-muted)]"
          :class="
            activeTab === 'overview'
              ? 'text-[var(--color-accent-strong)] border-b-[var(--color-accent)]'
              : ''
          "
          @click="activeTab = 'overview'"
        >
          {{ t("inspector.tabs.overview") }}
        </button>
        <button
          type="button"
          class="px-[var(--space-3)] py-[var(--space-1)] border-none bg-transparent text-[0.8125rem] font-medium text-[var(--color-text-muted)] cursor-pointer border-b-2 border-transparent -mb-px rounded-t-[var(--radius-sm)] transition-colors duration-150 hover:text-[var(--color-text-main)] hover:bg-[var(--color-surface-muted)]"
          :class="
            activeTab === 'files'
              ? 'text-[var(--color-accent-strong)] border-b-[var(--color-accent)]'
              : ''
          "
          @click="activeTab = 'files'"
        >
          {{ t("inspector.tabs.files") }}
        </button>
        <button
          v-if="isBtTask"
          type="button"
          class="px-[var(--space-3)] py-[var(--space-1)] border-none bg-transparent text-[0.8125rem] font-medium text-[var(--color-text-muted)] cursor-pointer border-b-2 border-transparent -mb-px rounded-t-[var(--radius-sm)] transition-colors duration-150 hover:text-[var(--color-text-main)] hover:bg-[var(--color-surface-muted)]"
          :class="
            activeTab === 'peersTrackers'
              ? 'text-[var(--color-accent-strong)] border-b-[var(--color-accent)]'
              : ''
          "
          @click="activeTab = 'peersTrackers'"
        >
          {{ t("inspector.tabs.peersTrackers") }}
        </button>
      </div>

      <!-- Tab content -->
      <div class="flex-1 overflow-auto min-h-0 p-[var(--space-3)]">
        <!-- ── Overview tab ── -->
        <div v-show="activeTab === 'overview'" class="grid gap-3">
          <div class="grid gap-[0.65rem]">
            <div>
              <div class="flex items-start justify-between gap-[var(--space-3)] flex-wrap">
                <h3 class="m-0 text-[var(--color-heading)] text-[var(--font-size-body)]">
                  {{ selectedOverview.fileName }}
                </h3>
                <UiBadge :tone="stateTone">{{ t(`states.${selectedOverview.state}`) }}</UiBadge>
              </div>
              <p class="m-0 text-[var(--color-text-muted)] text-[0.8rem] font-[var(--font-mono)]">
                {{ selectedOverview.destinationPath }}
              </p>
            </div>

            <div class="grid grid-cols-2 gap-[0.55rem] max-md:grid-cols-1">
              <div
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.transferred") }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                >
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
              <div
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.speed") }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                  >{{ formatSpeed(selectedOverview.speedBytesPerSecond) }}</span
                >
              </div>
              <div
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.eta") }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                  >{{ formatEta(selectedOverview.etaSeconds) }}</span
                >
              </div>
              <div
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{
                    selectedOverview.kind === "bt" ? t("inspector.peers") : t("inspector.threads")
                  }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                >
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
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.fields.seedCount") }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                  >{{ selectedOverview.seedCount ?? 0 }} /
                  {{ selectedOverview.leechCount ?? 0 }}</span
                >
              </div>
              <div
                v-if="selectedOverview.cdnAccelerated"
                class="flex flex-col gap-[0.15rem] p-[0.6rem_0.7rem] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] border border-[var(--color-border)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)] hover:border-[var(--color-border-strong)]"
              >
                <span
                  class="text-[var(--color-text-muted)] text-[0.7rem] font-medium tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.cdnNode") }}</span
                >
                <span
                  class="text-[var(--color-text-main)] text-[var(--font-size-metric)] font-[var(--font-weight-semibold)] leading-[1.35] break-all"
                >
                  <span class="i-ri-flashlight-fill" aria-hidden="true" />
                  {{ selectedOverview.cdnNodeIp || t("inspector.cdnAccelerated") }}
                </span>
              </div>
            </div>

            <div class="grid gap-[0.3rem]">
              <div
                class="flex justify-between gap-[var(--space-3)] text-[var(--color-text-muted)] text-[0.72rem]"
              >
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
            class="text-[var(--color-text-muted)] text-[0.72rem]"
          >
            {{
              t("inspector.chunkProgressText", {
                completed: selectedSnapshot.chunks.filter((c) => c.completed).length,
                total: selectedSnapshot.chunks.length,
              })
            }}
          </div>

          <dl
            v-if="showDetailInfo && detailRows.length"
            class="m-0 grid grid-cols-2 gap-[0.35rem_0.9rem] p-[0.65rem] border border-[var(--color-border)] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] max-md:grid-cols-1"
          >
            <div
              v-for="row in detailRows"
              :key="row.label"
              class="flex gap-[0.45rem] items-start min-w-0"
              :class="[row.wide ? 'col-span-full max-md:col-auto' : '']"
            >
              <dt
                class="m-0 text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
              >
                {{ row.label }}:
              </dt>
              <dd
                class="m-0 text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45]"
                :class="row.mono ? 'font-[var(--font-mono)]' : ''"
              >
                {{ row.value }}
              </dd>
            </div>
          </dl>

          <div
            v-if="selectedOverview.error"
            class="m-0 flex items-start gap-[var(--space-3)] border-l-3 border-l-[var(--color-danger-text)] shadow-[var(--shadow-card)]"
          >
            <span
              class="shrink-0 mt-[0.15rem] text-[1.1rem] leading-none i-ri-error-warning-line"
              aria-hidden="true"
            />
            <span class="flex-1 leading-[1.6]">{{ toFriendlyError(selectedOverview.error) }}</span>
          </div>
        </div>

        <!-- ── Files tab ── -->
        <div v-show="activeTab === 'files'" class="grid gap-3">
          <template v-if="selectedOverview.kind === 'http'">
            <dl
              class="m-0 grid grid-cols-2 gap-[0.35rem_0.9rem] p-[0.65rem] border border-[var(--color-border)] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] max-md:grid-cols-1"
            >
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.filename") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45]"
                  >{{ selectedOverview.fileName }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.destination") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedOverview.destinationPath }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.url") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedOverview.url }}</span
                >
              </div>
              <div
                v-if="selectedSnapshot?.finalUrl"
                class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto"
              >
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.fields.finalUrl") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedSnapshot.finalUrl }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.transferred") }}:</span
                >
                <span class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45]">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
            </dl>
          </template>
          <template v-else>
            <dl
              class="m-0 grid grid-cols-2 gap-[0.35rem_0.9rem] p-[0.65rem] border border-[var(--color-border)] rounded-[var(--radius-md)] bg-[var(--color-panel-muted)] max-md:grid-cols-1"
            >
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.filename") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45]"
                  >{{ selectedOverview.fileName }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.destination") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedOverview.destinationPath }}</span
                >
              </div>
              <div
                v-if="selectedOverview.infoHash"
                class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto"
              >
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.fields.infoHash") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedOverview.infoHash }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0 col-span-full max-md:col-auto">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.files.url") }}:</span
                >
                <span
                  class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45] font-[var(--font-mono)]"
                  >{{ selectedOverview.url }}</span
                >
              </div>
              <div class="flex gap-[0.45rem] items-start min-w-0">
                <span
                  class="text-[var(--color-text-muted)] font-medium whitespace-nowrap text-[0.7rem] tracking-[var(--letter-spacing-wide)] uppercase"
                  >{{ t("inspector.transferred") }}:</span
                >
                <span class="text-[var(--color-text-main)] break-all text-[0.8rem] leading-[1.45]">
                  {{ formatBytes(selectedOverview.downloadedBytes) }} /
                  {{ formatBytes(selectedOverview.totalBytes) }}
                </span>
              </div>
            </dl>

            <section class="grid gap-[0.65rem] pt-[0.5rem] border-t border-[var(--color-border)]">
              <div class="flex justify-between items-center gap-[var(--space-2)]">
                <h3
                  class="m-0 text-[0.72rem] font-semibold text-[var(--color-text-muted)] tracking-[var(--letter-spacing-wide)] uppercase"
                >
                  {{ t("inspector.files.fileCount", { count: btFileList.length }) }}
                </h3>
                <UiButton
                  variant="ghost"
                  size="sm"
                  icon="i-ri-refresh-line"
                  :loading="isFetchingBtFiles"
                  @click="fetchBtFiles()"
                >
                  {{ t("inspector.files.refreshFiles") }}
                </UiButton>
              </div>

              <div
                v-if="isFetchingBtFiles && btFileList.length === 0"
                class="m-0 p-4 text-center text-[var(--color-text-muted)] text-[0.8rem] border border-dashed border-[var(--color-border-strong)] rounded-[var(--radius-md)]"
              >
                {{ t("inspector.files.loadingFiles") }}
              </div>

              <div
                v-else-if="btFileList.length === 0"
                class="m-0 p-4 text-center text-[var(--color-text-muted)] text-[0.8rem] border border-dashed border-[var(--color-border-strong)] rounded-[var(--radius-md)]"
              >
                {{ t("inspector.files.noFileList") }}
              </div>

              <div
                v-else
                class="grid gap-[0.35rem] max-h-[360px] overflow-y-auto border border-[var(--color-border)] rounded-[var(--radius-md)] p-[var(--space-1)] bg-[var(--color-panel-muted)]"
              >
                <div
                  v-for="file in btFileList"
                  :key="file.index"
                  class="flex gap-[var(--space-2)] items-start px-[var(--space-2)] py-[var(--space-1)] rounded-[var(--radius-sm)] transition-colors duration-150 hover:bg-[var(--color-surface-muted)]"
                  :class="{ 'op-55': !file.included }"
                >
                  <label class="flex items-center pt-[0.15rem] cursor-pointer" @click.stop>
                    <input
                      type="checkbox"
                      :checked="file.included"
                      :disabled="isUpdatingFiles"
                      class="accent-[var(--color-accent)] cursor-pointer"
                      @change="toggleFileInclusion(file.index, file.included)"
                    />
                  </label>
                  <div class="flex-1 min-w-0 grid gap-[0.2rem]">
                    <span
                      class="text-[0.78rem] font-[var(--font-mono)] text-[var(--color-text-main)] break-all leading-[1.35]"
                      >{{ file.path }}</span
                    >
                    <div
                      class="text-[0.68rem] text-[var(--color-text-muted)] flex gap-[0.3rem] items-center flex-wrap"
                    >
                      <span>{{ formatBytes(file.size) }}</span>
                      <span class="text-[var(--color-border-strong)]">·</span>
                      <span>{{ formatBytes(file.downloadedBytes) }}</span>
                      <span class="text-[var(--color-border-strong)]">·</span>
                      <span v-if="file.included" class="text-[var(--color-success)]">{{
                        t("inspector.files.included")
                      }}</span>
                      <span v-else class="text-[var(--color-text-muted)]">{{
                        t("inspector.files.excluded")
                      }}</span>
                    </div>
                    <UiProgress
                      :value="file.size > 0 ? (file.downloadedBytes / file.size) * 100 : 0"
                      class="max-w-full"
                    />
                  </div>
                </div>
              </div>
            </section>
          </template>
        </div>

        <!-- ── Peers & Trackers tab (BT only) ── -->
        <div v-show="activeTab === 'peersTrackers'" v-if="isBtTask" class="grid gap-3">
          <div v-if="pieceList.length" class="text-[var(--color-text-muted)] text-[0.72rem]">
            {{
              t("inspector.pieceProgressText", {
                completed: pieceList.filter((p) => p.completed).length,
                total: pieceList.length,
              })
            }}
          </div>

          <section class="grid gap-[0.65rem] pt-[0.5rem] border-t border-[var(--color-border)]">
            <div class="flex justify-between items-center gap-[var(--space-2)]">
              <h3
                class="m-0 text-[0.72rem] font-semibold text-[var(--color-text-muted)] tracking-[var(--letter-spacing-wide)] uppercase"
              >
                {{ t("inspector.sections.peers") }} ({{ peerList.length }})
              </h3>
              <UiButton
                variant="ghost"
                size="sm"
                icon="i-ri-refresh-line"
                :loading="isFetchingPeers"
                @click="fetchBtPeers()"
              >
                {{ t("inspector.refreshPeers") }}
              </UiButton>
            </div>
            <BtPeerTable :peers="peerList" />
          </section>

          <section class="grid gap-[0.65rem] pt-[0.5rem] border-t border-[var(--color-border)]">
            <div class="flex justify-between items-center gap-[var(--space-2)]">
              <h3
                class="m-0 text-[0.72rem] font-semibold text-[var(--color-text-muted)] tracking-[var(--letter-spacing-wide)] uppercase"
              >
                {{ t("inspector.sections.trackers") }} ({{ trackerList.length }})
              </h3>
              <UiButton
                variant="ghost"
                size="sm"
                icon="i-ri-refresh-line"
                :loading="isFetchingTrackers"
                @click="fetchBtTrackers()"
              >
                {{ t("inspector.refreshTrackers") }}
              </UiButton>
            </div>
            <BtTrackerTable :trackers="trackerList" />
            <p
              v-if="btErrors.trackers"
              class="mt-[var(--space-1)] text-[0.75rem] text-[var(--color-text-error)]"
            >
              {{ btErrors.trackers }}
            </p>
          </section>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
/* All styling migrated to UnoCSS utility classes in template. */
/* Media query handled via max-md: responsive prefix. */
</style>
