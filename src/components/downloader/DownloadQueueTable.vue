<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { debounce, throttle } from "es-toolkit";

import { formatBytes, formatEta, formatSpeed, isSizeUnknown, progressLabel, progressValue } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import type { DownloadSummary } from "../../types/download";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";

type ColumnKey = "file" | "status" | "progress" | "speed" | "eta";

const props = defineProps<{
  downloads: DownloadSummary[];
  isAutoRefreshing: boolean;
  isRefreshingList: boolean;
  selectedId: string | null;
  taskActionName: string;
}>();

const emit = defineEmits<{
  copyLink: [downloadId: string];
  deleteTask: [downloadId: string];
  deleteTaskPermanently: [downloadId: string];
  openInExplorer: [downloadId: string];
  pauseOrResume: [downloadId: string];
  refresh: [];
  select: [downloadId: string];
  setBtSpeedLimit: [downloadId: string];
}>();

const pageSize = 10;
const syncShowDelayMs = 240;
const syncHideDelayMs = 420;
const { t } = useI18n();
const currentPage = ref(1);
const columnMenuOpen = ref(false);
const contextMenu = ref<{ downloadId: string; x: number; y: number } | null>(null);
const isRefreshLabelVisible = ref(false);
const isSyncIndicatorVisible = ref(false);
const visibleColumns = ref<ColumnKey[]>(["file", "status", "progress", "speed", "eta"]);

const columnOptions = computed<Array<{ key: ColumnKey; label: string; alwaysVisible?: boolean }>>(
  () => [
    { key: "file", label: t("queue.file"), alwaysVisible: true },
    { key: "status", label: t("queue.status") },
    { key: "progress", label: t("queue.progress") },
    { key: "speed", label: t("queue.speed") },
    { key: "eta", label: t("queue.eta") },
  ],
);

const totalPages = computed(() => Math.max(1, Math.ceil(props.downloads.length / pageSize)));
const pagedDownloads = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  return props.downloads.slice(start, start + pageSize);
});
const pageStart = computed(() =>
  props.downloads.length ? (currentPage.value - 1) * pageSize + 1 : 0,
);
const pageEnd = computed(() =>
  props.downloads.length ? Math.min(currentPage.value * pageSize, props.downloads.length) : 0,
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
  () => props.downloads.length,
  () => {
    if (currentPage.value > totalPages.value) {
      currentPage.value = totalPages.value;
    }
  },
);

watch(
  () => props.downloads,
  (downloads) => {
    if (!contextMenu.value) {
      return;
    }

    if (!downloads.some((download) => download.id === contextMenu.value?.downloadId)) {
      contextMenu.value = null;
    }
  },
  { deep: true },
);

watch(
  () => props.taskActionName,
  (value) => {
    if (value) {
      contextMenu.value = null;
    }
  },
);

const syncRefreshIndicator = debounce((value: boolean) => {
  isRefreshLabelVisible.value = value;
}, 140);

const showAutoRefreshIndicator = debounce(() => {
  isSyncIndicatorVisible.value = true;
}, syncShowDelayMs);

const hideAutoRefreshIndicator = debounce(() => {
  isSyncIndicatorVisible.value = false;
}, syncHideDelayMs);

watch(
  () => props.isRefreshingList,
  (value) => {
    syncRefreshIndicator(value);
  },
  { immediate: true },
);

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

function toneForState(state: DownloadSummary["state"]): "info" | "success" | "warning" | "danger" {
  if (state === "completed") return "success";
  if (state === "failed" || state === "canceled") return "danger";
  if (state === "queued" || state === "paused") return "warning";
  return "info";
}

function isColumnVisible(key: ColumnKey) {
  return visibleColumns.value.includes(key);
}

function toggleColumn(key: ColumnKey) {
  const option = columnOptions.value.find((item) => item.key === key);
  if (option?.alwaysVisible) {
    return;
  }

  if (visibleColumns.value.includes(key)) {
    visibleColumns.value = visibleColumns.value.filter((column) => column !== key);
    return;
  }

  visibleColumns.value = columnOptions.value
    .map((item) => item.key)
    .filter((column) => column === key || visibleColumns.value.includes(column));
}

function goToPreviousPage() {
  currentPage.value = Math.max(1, currentPage.value - 1);
}

function goToNextPage() {
  currentPage.value = Math.min(totalPages.value, currentPage.value + 1);
}

function closeMenus() {
  columnMenuOpen.value = false;
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
  columnMenuOpen.value = false;

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

const triggerRefresh = throttle(() => {
  emit("refresh");
}, 600);


function labelForTaskKind(kind: DownloadSummary["kind"]) {
  if (kind === "bt") {
    return t("tokens.bt");
  }

  if (kind === "metalink") {
    return t("tokens.metalink");
  }

  if (kind === "sftp") {
    return t("tokens.sftp");
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
        <div class="column-menu">
          <UiButton
            type="button"
            size="sm"
            variant="ghost"
            icon="i-ri-layout-column-line"
            @click.stop="
              contextMenu = null;
              columnMenuOpen = !columnMenuOpen;
            "
          >
            {{ t("queue.columns") }}
          </UiButton>
          <div v-if="columnMenuOpen" class="column-menu__panel" @pointerdown.stop>
            <label
              v-for="column in columnOptions"
              :key="column.key"
              class="column-menu__item"
              :class="{
                'column-menu__item--checked': isColumnVisible(column.key),
                'column-menu__item--locked': column.alwaysVisible,
              }"
            >
              <input
                :checked="isColumnVisible(column.key)"
                type="checkbox"
                :disabled="column.alwaysVisible"
                @change="toggleColumn(column.key)"
              />
              <span
                class="column-menu__indicator"
                :class="isColumnVisible(column.key) ? 'i-ri-check-line' : 'i-ri-add-line'"
                aria-hidden="true"
              />
              <span>{{ column.label }}</span>
            </label>
          </div>
        </div>
        <UiButton
          type="button"
          size="sm"
          variant="secondary"
          icon="i-ri-refresh-line"
          @click="triggerRefresh"
        >
          {{ isRefreshLabelVisible ? t("common.refreshing") : t("common.refresh") }}
        </UiButton>
      </div>
    </div>

    <div v-if="downloads.length" class="queue-panel__table">
      <div class="queue-table-shell">
        <table class="queue-table">
          <thead>
            <tr>
              <th v-if="isColumnVisible('file')">{{ t("queue.file") }}</th>
              <th v-if="isColumnVisible('status')">{{ t("queue.status") }}</th>
              <th v-if="isColumnVisible('progress')">{{ t("queue.progress") }}</th>
              <th v-if="isColumnVisible('speed')">{{ t("queue.speed") }}</th>
              <th v-if="isColumnVisible('eta')">{{ t("queue.eta") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="download in pagedDownloads"
              :key="download.id"
              class="queue-row"
              :class="{ 'queue-row--active': download.id === selectedId }"
              @click="$emit('select', download.id)"
              @contextmenu.prevent.stop="openTaskContextMenu($event, download.id)"
            >
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

              <td v-if="isColumnVisible('speed')" class="queue-cell queue-cell--meta">
                {{ formatSpeed(download.speedBytesPerSecond) }}
              </td>

              <td v-if="isColumnVisible('eta')" class="queue-cell queue-cell--meta">
                {{ formatEta(download.etaSeconds) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="queue-pagination">
        <p class="queue-pagination__summary">
          {{ t("queue.showing", { start: pageStart, end: pageEnd, total: downloads.length }) }}
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
            <button
              type="button"
              class="task-context-menu__item"
              @click="onSetBtSpeedLimit"
            >
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

.column-menu {
  position: relative;
}

.column-menu__panel {
  position: absolute;
  top: calc(100% + 0.35rem);
  right: 0;
  z-index: 5;
  min-width: 9rem;
  display: grid;
  gap: 0.15rem;
  padding: 0.35rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
  box-shadow: var(--shadow-card);
}

.column-menu__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.35rem 0.45rem;
  border-radius: var(--radius-sm);
  border: 1px solid transparent;
  color: var(--color-text-main);
  font-size: var(--font-size-small);
  cursor: pointer;
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.column-menu__item:hover {
  background: var(--color-surface-muted);
}

.column-menu__item--checked {
  background: var(--color-accent-soft);
  border-color: var(--color-accent-soft-border);
  color: var(--color-accent-strong);
}

.column-menu__item--locked {
  cursor: not-allowed;
  opacity: 0.6;
}

.column-menu__indicator {
  width: 1rem;
  display: inline-flex;
  justify-content: center;
  color: inherit;
  font-size: 0.9rem;
}

.column-menu__item input {
  width: 0.9rem;
  height: 0.9rem;
  margin: 0;
  accent-color: var(--color-accent);
}

.column-menu__item span:last-child {
  flex: 1;
}

.queue-panel__table {
  display: grid;
  gap: 0.625rem;
}

.queue-table-shell {
  min-height: 27.5rem;
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
  height: 2.5rem;
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

.queue-cell {
  padding: 0.3rem 0.75rem;
  vertical-align: middle;
}

.queue-cell--file {
  width: 32%;
}

.queue-cell--status {
  width: 12%;
}

.queue-cell--progress {
  width: 30%;
}

.queue-cell--meta {
  width: 13%;
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
    min-width: 48rem;
  }
}
</style>
