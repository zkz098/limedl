<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { debounce } from "es-toolkit";

import { filterDownloads } from "../../lib/download-filter";
import {
  formatBytes,
  formatEta,
  formatSpeed,
  isSizeUnknown,
  progressLabel,
  progressValue,
} from "../../lib/download-format";
import { useI18n } from "../../i18n";
import type { ColumnKey } from "../../lib/column-defs";
import type { DownloadSummary } from "../../types/download";
import type { ViewOptions, MultiSelectState } from "./queue-types";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";

const props = defineProps<{
  downloads: DownloadSummary[];
  selectedId: string | null;
  taskActionName: string;
  isAutoRefreshing: boolean;
  stateFilter?: string;
  searchQuery?: string;
  viewOptions: ViewOptions;
  multiSelect: MultiSelectState;
}>();

const emit = defineEmits<{
  copyLink: [downloadId: string];
  deleteTask: [downloadId: string];
  deleteTaskPermanently: [downloadId: string];
  openInExplorer: [downloadId: string];
  pauseOrResume: [downloadId: string];
  select: [downloadId: string];
  setBtSpeedLimit: [downloadId: string];
  toggleSelect: [downloadId: string];
}>();

const pageSize = 20;
const syncShowDelayMs = 240;
const syncHideDelayMs = 420;
const { t } = useI18n();
const currentPage = ref(1);
const contextMenu = ref<{ downloadId: string; x: number; y: number } | null>(null);
const isSyncIndicatorVisible = ref(false);

const columnOptions = computed<Array<{ key: ColumnKey; label: string; alwaysVisible?: boolean }>>(
  () => [
    { key: "file", label: t("queue.file"), alwaysVisible: true },
    { key: "size", label: t("queue.size") },
    { key: "downloaded", label: t("queue.downloaded") },
    { key: "status", label: t("queue.status") },
    { key: "progress", label: t("queue.progress") },
    { key: "speed", label: t("queue.speed") },
    { key: "uploadSpeed", label: t("queue.upSpeed") },
    { key: "seeds", label: t("queue.seeds") },
    { key: "eta", label: t("queue.eta") },
  ],
);

const visibleColumnKeys = computed(() => new Set(props.viewOptions.visibleColumns));
const visibleColumnsOrdered = computed(() =>
  columnOptions.value.filter((column) => visibleColumnKeys.value.has(column.key)),
);

const filteredDownloads = computed(() =>
  filterDownloads(props.downloads, props.searchQuery ?? "", props.stateFilter ?? ""),
);

const sortedDownloads = computed(() => {
  const list = [...filteredDownloads.value];
  const direction = props.viewOptions.sortDirection === "asc" ? 1 : -1;

  list.sort((a, b) => {
    let comparison = 0;

    switch (props.viewOptions.sortKey) {
      case "name":
        comparison = a.fileName.localeCompare(b.fileName);
        break;
      case "size":
        comparison = (a.totalBytes ?? 0) - (b.totalBytes ?? 0);
        break;
      case "progress":
        comparison = progressValue(a) - progressValue(b);
        break;
      case "speed":
        comparison = (a.speedBytesPerSecond ?? 0) - (b.speedBytesPerSecond ?? 0);
        break;
      case "added_at":
        comparison = (a.createdAtMs ?? 0) - (b.createdAtMs ?? 0);
        break;
      case "state":
        comparison = a.state.localeCompare(b.state);
        break;
    }

    return comparison * direction;
  });

  return list;
});

const totalPages = computed(() => Math.max(1, Math.ceil(sortedDownloads.value.length / pageSize)));
const pagedDownloads = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  return sortedDownloads.value.slice(start, start + pageSize);
});
const pageStart = computed(() =>
  sortedDownloads.value.length ? (currentPage.value - 1) * pageSize + 1 : 0,
);
const pageEnd = computed(() =>
  sortedDownloads.value.length
    ? Math.min(currentPage.value * pageSize, sortedDownloads.value.length)
    : 0,
);
const contextMenuDownload = computed(
  () => props.downloads.find((download) => download.id === contextMenu.value?.downloadId) ?? null,
);
const canTogglePauseOrResume = computed(() => {
  const state = contextMenuDownload.value?.state;
  return Boolean(
    state &&
    (["paused", "failed"].includes(state) ||
      ["queued", "downloading", "retrying", "verifying"].includes(state)),
  );
});
const contextActionLabel = computed(() => {
  if (["paused", "failed"].includes(contextMenuDownload.value?.state ?? "")) {
    return t("queue.continue");
  }

  if (canTogglePauseOrResume.value) {
    return t("queue.pause");
  }

  return t("queue.pauseOrResume");
});
const contextActionIcon = computed(() =>
  ["paused", "failed"].includes(contextMenuDownload.value?.state ?? "")
    ? "i-ri-play-line"
    : "i-ri-pause-line",
);

watch(
  () => sortedDownloads.value.length,
  () => {
    if (currentPage.value > totalPages.value) {
      currentPage.value = totalPages.value;
    }
  },
);

watch(
  () => props.multiSelect.removedDownloadIds,
  (ids) => {
    if (!contextMenu.value || ids.length === 0) {
      return;
    }

    if (ids.includes(contextMenu.value.downloadId)) {
      contextMenu.value = null;
    }
  },
);

watch(
  () => props.taskActionName,
  (value) => {
    if (value) {
      contextMenu.value = null;
    }
  },
);

const showAutoRefreshIndicator = debounce(() => {
  isSyncIndicatorVisible.value = true;
}, syncShowDelayMs);

const hideAutoRefreshIndicator = debounce(() => {
  isSyncIndicatorVisible.value = false;
}, syncHideDelayMs);

watch(
  () => props.isAutoRefreshing,
  (value) => {
    if (value) {
      hideAutoRefreshIndicator.cancel();
      showAutoRefreshIndicator();
      return;
    }

    showAutoRefreshIndicator.cancel();
    hideAutoRefreshIndicator();
  },
  { immediate: true },
);

const allPageSelected = computed(() => {
  if (!props.multiSelect.multiSelectMode || pagedDownloads.value.length === 0) return false;
  return pagedDownloads.value.every((d) => props.multiSelect.selectedIds.has(d.id));
});

function toggleSelectAllOnPage() {
  if (allPageSelected.value) {
    // Deselect all on this page
    pagedDownloads.value.forEach((d) => {
      if (props.multiSelect.selectedIds.has(d.id)) {
        emit("toggleSelect", d.id);
      }
    });
  } else {
    // Select all on this page
    pagedDownloads.value.forEach((d) => {
      if (!props.multiSelect.selectedIds.has(d.id)) {
        emit("toggleSelect", d.id);
      }
    });
  }
}

function toneForState(state: DownloadSummary["state"]): "info" | "success" | "warning" | "danger" {
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
}

function isColumnVisible(key: ColumnKey) {
  return props.viewOptions.visibleColumns.includes(key);
}

function goToPreviousPage() {
  currentPage.value = Math.max(1, currentPage.value - 1);
}

function goToNextPage() {
  currentPage.value = Math.min(totalPages.value, currentPage.value + 1);
}

function closeMenus() {
  contextMenu.value = null;
}

function clampMenuPosition(clientX: number, clientY: number) {
  const menuWidth = 220;
  const menuHeight = 280;
  const gutter = 12;

  return {
    x: Math.max(gutter, Math.min(clientX, window.innerWidth - menuWidth - gutter)),
    y: Math.max(gutter, Math.min(clientY, window.innerHeight - menuHeight - gutter)),
  };
}

function openTaskContextMenu(event: MouseEvent, downloadId: string) {
  emit("select", downloadId);

  const { x, y } = clampMenuPosition(event.clientX, event.clientY);
  contextMenu.value = { downloadId, x, y };
}

function handleGlobalPointerDown() {
  closeMenus();
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeMenus();
  }
}

function handlePauseOrResume() {
  if (!contextMenu.value || !canTogglePauseOrResume.value) {
    return;
  }

  emit("pauseOrResume", contextMenu.value.downloadId);
  contextMenu.value = null;
}

function handleDeleteTask() {
  if (!contextMenu.value) {
    return;
  }

  emit("deleteTask", contextMenu.value.downloadId);
  contextMenu.value = null;
}

function handleDeleteTaskPermanently() {
  if (!contextMenu.value) {
    return;
  }

  emit("deleteTaskPermanently", contextMenu.value.downloadId);
  contextMenu.value = null;
}

function handleCopyLink() {
  if (!contextMenu.value) {
    return;
  }

  emit("copyLink", contextMenu.value.downloadId);
  contextMenu.value = null;
}

function handleOpenInExplorer() {
  if (!contextMenu.value) {
    return;
  }

  emit("openInExplorer", contextMenu.value.downloadId);
  contextMenu.value = null;
}

function onSetBtSpeedLimit() {
  if (!contextMenuDownload.value) {
    return;
  }

  emit("setBtSpeedLimit", contextMenuDownload.value.id);
  contextMenu.value = null;
}

function labelForTaskKind(kind: DownloadSummary["kind"]) {
  if (kind === "bt") {
    return t("tokens.bt");
  }

  return t("tokens.http");
}

function labelForUploadStatus(status?: DownloadSummary["uploadStatus"]) {
  return status ? t(`uploadStatus.${status}`) : t("uploadStatus.idle");
}

function metaForDownload(download: DownloadSummary) {
  if (download.kind === "bt") {
    const parts: string[] = [labelForUploadStatus(download.uploadStatus)];
    const hasSeedCount = download.seedCount != null;
    const hasLeechCount = download.leechCount != null;
    // TODO L1: backend always returns null for seed/leech; consider hiding or using t('common.dash') here
    if (hasSeedCount || hasLeechCount) {
      parts.push(`${download.seedCount ?? "—"} S / ${download.leechCount ?? "—"} L`);
    }
    parts.push(t("queue.peerCount", { count: download.peerCount ?? 0 }));
    parts.push(t("queue.uploaded", { size: formatBytes(download.uploadedBytes) }));
    return parts.join(" · ");
  }

  const parts = [
    download.threadMode === "adaptive" ? t("queue.adaptive") : t("queue.fixedThread"),
    t("queue.currentThreads", { count: download.connectionCount }),
  ];

  if (download.adaptiveProfile) {
    parts.push(t(`tokens.${download.adaptiveProfile}`));
  }

  if (download.threadNote) {
    parts.push(download.threadNote);
  }

  return parts.join(" · ");
}

onMounted(() => {
  window.addEventListener("pointerdown", handleGlobalPointerDown);
  window.addEventListener("resize", closeMenus);
  window.addEventListener("scroll", closeMenus, true);
  window.addEventListener("keydown", handleEscape);
});

onUnmounted(() => {
  window.removeEventListener("pointerdown", handleGlobalPointerDown);
  window.removeEventListener("resize", closeMenus);
  window.removeEventListener("scroll", closeMenus, true);
  window.removeEventListener("keydown", handleEscape);
});
</script>

<template>
  <section class="queue-panel">
    <div class="desk-panel__header queue-panel__header">
      <div>
        <p class="section-kicker">{{ t("queue.kicker") }}</p>
        <h2 class="panel-title">{{ t("queue.title") }}</h2>
      </div>

      <div class="queue-panel__actions">
        <span class="sync-pill" :data-active="isSyncIndicatorVisible">{{
          isSyncIndicatorVisible ? t("queue.autoSyncing") : t("queue.idle")
        }}</span>
      </div>
    </div>

    <div v-if="sortedDownloads.length" class="queue-panel__table">
      <div class="queue-table-shell">
        <table class="queue-table" :class="{ 'queue-table--compact': viewOptions.compactView }">
          <thead>
            <tr>
              <th v-if="multiSelect.multiSelectMode" class="queue-cell--checkbox">
                <input type="checkbox" :checked="allPageSelected" @change="toggleSelectAllOnPage" />
              </th>
              <th v-for="column in visibleColumnsOrdered" :key="column.key">{{ column.label }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="download in pagedDownloads"
              :key="download.id"
              class="queue-row"
              :class="{
                'queue-row--active': !multiSelect.multiSelectMode && download.id === selectedId,
                'queue-row--selected': multiSelect.multiSelectMode && multiSelect.selectedIds.has(download.id),
              }"
              @click="
                multiSelect.multiSelectMode ? $emit('toggleSelect', download.id) : $emit('select', download.id)
              "
              @contextmenu.prevent.stop="openTaskContextMenu($event, download.id)"
            >
              <td v-if="multiSelect.multiSelectMode" class="queue-cell queue-cell--checkbox">
                <input
                  type="checkbox"
                  :checked="multiSelect.selectedIds.has(download.id)"
                  @change="$emit('toggleSelect', download.id)"
                />
              </td>
              <td v-if="isColumnVisible('file')" class="queue-cell queue-cell--file">
                <div class="queue-file">
                  <span class="queue-file__title">
                    <span class="queue-file__name">{{ download.fileName }}</span>
                    <span class="queue-file__kind">{{ labelForTaskKind(download.kind) }}</span>
                    <UiBadge
                      v-if="download.cdnAccelerated"
                      size="sm"
                      tone="warning"
                      class="queue-file__cdn"
                    >
                      <span class="i-ri-flashlight-fill" aria-hidden="true" />
                      CDN
                    </UiBadge>
                  </span>
                  <span class="queue-file__path">{{ download.destinationPath }}</span>
                  <span class="queue-file__meta">{{ metaForDownload(download) }}</span>
                </div>
              </td>

              <td v-if="isColumnVisible('size')" class="queue-cell queue-cell--size">
                {{ formatBytes(download.totalBytes) }}
              </td>

              <td v-if="isColumnVisible('downloaded')" class="queue-cell queue-cell--downloaded">
                {{ formatBytes(download.downloadedBytes) }}
              </td>

              <td v-if="isColumnVisible('status')" class="queue-cell queue-cell--status">
                <UiBadge size="sm" :tone="toneForState(download.state)">{{
                  t(`states.${download.state}`)
                }}</UiBadge>
              </td>

              <td v-if="isColumnVisible('progress')" class="queue-cell queue-cell--progress">
                <div class="queue-progress">
                  <div class="queue-progress__copy">
                    <span>{{ progressLabel(download) }}</span>
                    <span>
                      {{ formatBytes(download.downloadedBytes) }} /
                      {{ formatBytes(download.totalBytes) }}
                    </span>
                  </div>
                  <UiProgress
                    :value="progressValue(download)"
                    :indeterminate="isSizeUnknown(download) && download.state !== 'completed'"
                  />
                </div>
              </td>

              <td v-if="isColumnVisible('speed')" class="queue-cell queue-cell--speed">
                {{ formatSpeed(download.speedBytesPerSecond) }}
              </td>

              <td v-if="isColumnVisible('uploadSpeed')" class="queue-cell queue-cell--up-speed">
                {{
                  download.kind === "bt"
                    ? formatSpeed(download.uploadSpeedBytesPerSecond)
                    : "\u2014"
                }}
              </td>

              <td v-if="isColumnVisible('seeds')" class="queue-cell queue-cell--seeds">
                {{
                  download.kind === "bt"
                    ? `${download.seedCount ?? "\u2014"}/${download.leechCount ?? "\u2014"}`
                    : "\u2014"
                }}
              </td>

              <td v-if="isColumnVisible('eta')" class="queue-cell queue-cell--eta">
                {{ formatEta(download.etaSeconds) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="queue-pagination">
        <p class="queue-pagination__summary">
          {{
            t("queue.showing", { start: pageStart, end: pageEnd, total: sortedDownloads.length })
          }}
        </p>
        <div class="queue-pagination__actions">
          <UiButton
            type="button"
            size="sm"
            variant="ghost"
            icon="i-ri-arrow-left-s-line"
            :disabled="currentPage === 1"
            @click="goToPreviousPage"
          >
            {{ t("queue.previous") }}
          </UiButton>
          <span class="queue-pagination__page">{{
            t("queue.page", { current: currentPage, total: totalPages })
          }}</span>
          <UiButton
            type="button"
            size="sm"
            variant="ghost"
            icon-right="i-ri-arrow-right-s-line"
            :disabled="currentPage === totalPages"
            @click="goToNextPage"
          >
            {{ t("queue.next") }}
          </UiButton>
        </div>
      </div>

      <Teleport to="body">
        <div
          v-if="contextMenu && contextMenuDownload"
          class="task-context-menu"
          :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
          @pointerdown.stop
        >
          <button
            type="button"
            class="task-context-menu__item"
            :disabled="!canTogglePauseOrResume"
            @click="handlePauseOrResume"
          >
            <span :class="contextActionIcon" aria-hidden="true" />
            <span>{{ contextActionLabel }}</span>
          </button>
          <template v-if="contextMenuDownload?.kind === 'bt'">
            <button type="button" class="task-context-menu__item" @click="onSetBtSpeedLimit">
              <span class="i-ri-speed-up-line" aria-hidden="true" />
              <span>{{ t("queue.setSpeedLimit") }}</span>
            </button>
          </template>
          <button type="button" class="task-context-menu__item" @click="handleDeleteTask">
            <span class="i-ri-delete-bin-6-line" aria-hidden="true" />
            <span>{{ t("queue.deleteTask") }}</span>
          </button>
          <button type="button" class="task-context-menu__item" @click="handleCopyLink">
            <span class="i-ri-file-copy-line" aria-hidden="true" />
            <span>{{ t("queue.copyLink") }}</span>
          </button>
          <button
            type="button"
            class="task-context-menu__item task-context-menu__item--danger"
            @click="handleDeleteTaskPermanently"
          >
            <span class="i-ri-delete-bin-line" aria-hidden="true" />
            <span>{{ t("queue.permanentDelete") }}</span>
          </button>
          <button type="button" class="task-context-menu__item" @click="handleOpenInExplorer">
            <span class="i-ri-folder-open-line" aria-hidden="true" />
            <span>{{ t("queue.openInExplorer") }}</span>
          </button>
        </div>
      </Teleport>
    </div>

    <div v-else-if="downloads.length" class="queue-empty">
      <span class="queue-empty__icon i-ri-search-eye-line" aria-hidden="true" />
      <h3>{{ t("queue.noResults") }}</h3>
    </div>
    <div v-else class="queue-empty">
      <span class="queue-empty__icon i-ri-inbox-archive-line" aria-hidden="true" />
      <h3>{{ t("queue.emptyTitle") }}</h3>
      <p>{{ t("queue.emptyDescription") }}</p>
    </div>
  </section>
</template>

<style scoped>
.queue-panel {
  display: grid;
  gap: var(--space-4);
}

.queue-panel__header {
  padding: 0.25rem 0;
}

.queue-panel__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.sync-pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 1.75rem;
  min-width: 6.5rem;
  padding-inline: var(--space-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  color: var(--color-text-muted);
  font-size: var(--font-size-label);
  font-family: var(--font-mono);
  letter-spacing: 0;
  text-transform: none;
}

.sync-pill[data-active="true"] {
  color: var(--color-accent-strong);
  border-color: var(--color-accent-soft-border);
  background: var(--color-accent-soft);
}

.sync-pill:not([data-active="true"]) {
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease,
    color 0.2s ease;
}

.queue-panel__table {
  display: grid;
  gap: 0.625rem;
}

.queue-table-shell {
  min-height: 30rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--color-panel);
}

.queue-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.queue-table thead th {
  height: 2rem;
  padding: 0 0.75rem;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  color: var(--color-text-muted);
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: var(--letter-spacing-wide);
  text-align: left;
  text-transform: uppercase;
}

.queue-table tbody tr {
  height: auto;
  min-height: 3.5rem;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.queue-table tbody tr + tr td {
  border-top: 1px solid var(--color-border);
}

.queue-table tbody tr:hover {
  background: var(--color-surface-muted);
}

.queue-row--active {
  background: var(--color-accent-soft);
}

.queue-row--active td {
  border-top-color: var(--color-accent-soft-border);
}

.queue-row--selected {
  background: var(--color-surface-muted);
}

.queue-row--selected td {
  border-top-color: var(--color-border);
}

.queue-cell--checkbox {
  width: 2.5rem;
  text-align: center;
  vertical-align: middle;
  padding: 0;
}

.queue-cell--checkbox input {
  width: 1rem;
  height: 1rem;
  accent-color: var(--color-accent);
  cursor: pointer;
  margin: 0;
}

.queue-cell {
  padding: var(--space-1) var(--space-2);
  vertical-align: middle;
  font-size: 0.8125rem;
}

.queue-table--compact tbody tr {
  min-height: 2.75rem;
}

.queue-table--compact .queue-cell {
  padding: 0.125rem var(--space-1);
  font-size: 0.75rem;
}

.queue-table--compact .queue-file__name {
  font-size: 0.78rem;
}

.queue-table--compact .queue-file__path,
.queue-table--compact .queue-file__meta {
  font-size: 0.65rem;
}

.queue-cell--file {
  width: 24%;
}

.queue-cell--size {
  width: 8%;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-cell--downloaded {
  width: 10%;
  color: var(--color-text-soft);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-cell--status {
  width: 10%;
}

.queue-cell--progress {
  width: 18%;
}

.queue-cell--speed {
  width: 10%;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-cell--up-speed {
  width: 8%;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-cell--seeds {
  width: 6%;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-cell--eta {
  width: 6%;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  font-family: var(--font-mono);
}

.queue-file {
  display: grid;
  gap: 0.1rem;
  min-width: 0;
}

.queue-file__title {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  min-width: 0;
}

.queue-file__name {
  min-width: 0;
  color: var(--color-heading);
  font-weight: 600;
  font-size: 0.82rem;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.queue-file__kind {
  flex: 0 0 auto;
  padding: 0.05rem 0.3rem;
  border-radius: var(--radius-sm);
  border: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  color: var(--color-text-muted);
  font-size: 0.6rem;
  font-weight: 600;
  line-height: 1.3;
}

.queue-file__cdn {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  font-size: 0.6rem;
  font-weight: 600;
}

.queue-file__cdn .i-ri-flashlight-fill {
  font-size: 0.7rem;
}

.queue-file__path {
  color: var(--color-text-muted);
  font-size: 0.72rem;
  line-height: 1.2;
  font-family: var(--font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.queue-file__meta {
  color: var(--color-text-soft);
  font-size: 0.68rem;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.queue-progress {
  display: grid;
  gap: 0.2rem;
}

.queue-progress__copy {
  display: flex;
  justify-content: space-between;
  gap: var(--space-2);
  color: var(--color-text-muted);
  font-size: 0.7rem;
  font-family: var(--font-mono);
}

.queue-empty {
  display: grid;
  gap: var(--space-2);
  place-items: center;
  min-height: 18rem;
  text-align: center;
  color: var(--color-text-muted);
  border: 1px dashed var(--color-border-strong);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
}

.queue-empty h3,
.queue-empty p {
  margin: 0;
}

.queue-empty__icon {
  font-size: 1.75rem;
  color: var(--color-accent);
}

.queue-pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.queue-pagination__summary,
.queue-pagination__page {
  margin: 0;
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  font-family: var(--font-mono);
}

.queue-pagination__actions {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  flex-wrap: wrap;
}

.task-context-menu {
  position: fixed;
  z-index: 30;
  min-width: 12rem;
  display: grid;
  gap: 0.15rem;
  padding: 0.35rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
  box-shadow: var(--shadow-card-hover);
}

.task-context-menu__item {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-height: 2rem;
  padding: 0 0.6rem;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-main);
  cursor: pointer;
  font-size: var(--font-size-small);
  text-align: left;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.task-context-menu__item:hover:not(:disabled) {
  background: var(--color-surface-muted);
}

.task-context-menu__item:disabled {
  color: var(--color-text-soft);
  cursor: not-allowed;
}

.task-context-menu__item--danger {
  color: var(--color-danger-text);
}

.task-context-menu__item--danger:hover:not(:disabled) {
  background: var(--color-danger-bg);
}

@media (max-width: 1160px) {
  .queue-table-shell {
    overflow-x: auto;
  }

  .queue-table {
    min-width: 64rem;
  }
}
</style>
