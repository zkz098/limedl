<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { debounce } from "../../lib/debounce";
import { usePagination } from "../../composables/usePagination";

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
import { VALID_COLUMN_KEYS } from "../../lib/column-defs";
import type {
  DownloadSummary,
  ViewOptions,
  MultiSelectState,
  Priority,
} from "../../types/download";
import UiBadge from "../ui/UiBadge.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import UiEmptyState from "../ui/UiEmptyState.vue";
import UiSelect from "../ui/UiSelect.vue";
import { toneForState } from "../../composables/downloadHelpers";
import { useFloatingClose } from "../../composables/useFloatingClose";

const props = defineProps<{
  downloads: DownloadSummary[];
  selectedId: string | null;
  taskActionName: string;
  isAutoRefreshing: boolean;
  viewOptions: ViewOptions;
  multiSelect: MultiSelectState;
}>();

const emit = defineEmits<{
  copyLink: [downloadId: string];
  deleteTask: [downloadId: string];
  deleteTaskPermanently: [downloadId: string];
  doubleClick: [downloadId: string];
  newTask: [];
  openInExplorer: [downloadId: string];
  pauseOrResume: [downloadId: string];
  refresh: [];
  select: [downloadId: string];
  setBtSpeedLimit: [downloadId: string];
  setPriority: [downloadId: string, priority: Priority];
  toggleSelect: [downloadId: string];
}>();

const syncShowDelayMs = 240;
const syncHideDelayMs = 420;
const { t } = useI18n();
const contextMenu = ref<{ downloadId: string; x: number; y: number } | null>(null);
const contextMenuPanelRef = ref<HTMLElement | null>(null);
const backgroundMenu = ref<{ x: number; y: number } | null>(null);
const backgroundMenuPanelRef = ref<HTMLElement | null>(null);
const priorityMenu = ref<{ downloadId: string; x: number; y: number } | null>(null);
const priorityMenuRef = ref<HTMLElement | null>(null);
const isSyncIndicatorVisible = ref(false);

const priorityOptions = computed<Array<{ value: Priority; label: string }>>(() => [
  { value: "high", label: t("composer.priorityHigh") },
  { value: "normal", label: t("composer.priorityNormal") },
  { value: "low", label: t("composer.priorityLow") },
]);

function priorityTone(priority: Priority): "danger" | "neutral" | "info" {
  if (priority === "high") return "danger";
  if (priority === "low") return "info";
  return "neutral";
}

function priorityLabel(priority: Priority) {
  return t(`composer.priority${priority.charAt(0).toUpperCase() + priority.slice(1)}`);
}

function openPriorityMenu(event: MouseEvent, downloadId: string) {
  event.stopPropagation();
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const menuWidth = 120;
  const menuHeight = 110;
  const gutter = 12;
  const x = Math.max(gutter, Math.min(rect.left, window.innerWidth - menuWidth - gutter));
  const y = Math.max(gutter, Math.min(rect.bottom + 4, window.innerHeight - menuHeight - gutter));
  priorityMenu.value = { downloadId, x, y };
}

function closePriorityMenu() {
  priorityMenu.value = null;
}

function handleSetPriority(downloadId: string, priority: Priority) {
  emit("setPriority", downloadId, priority);
  priorityMenu.value = null;
}

const showPriorityMenu = computed(() => priorityMenu.value !== null);
useFloatingClose(priorityMenuRef, showPriorityMenu, closePriorityMenu);

const columnLabelMap: Record<ColumnKey, () => string> = {
  file: () => t("queue.file"),
  size: () => t("queue.size"),
  downloaded: () => t("queue.downloaded"),
  status: () => t("queue.status"),
  progress: () => t("queue.progress"),
  speed: () => t("queue.speed"),
  priority: () => t("queue.priorityColumn"),
  uploadSpeed: () => t("queue.upSpeed"),
  seeds: () => t("queue.seeds"),
  eta: () => t("queue.eta"),
};

const columnOptions = computed<Array<{ key: ColumnKey; label: string; alwaysVisible?: boolean }>>(
  () =>
    VALID_COLUMN_KEYS.map((key) => ({
      key,
      label: columnLabelMap[key](),
      alwaysVisible: key === "file",
    })),
);

const pageSizeOptions = computed(() => [
  { label: "20", value: 20 as number | null },
  { label: "50", value: 50 as number | null },
  { label: "100", value: 100 as number | null },
  { label: t("queue.showAll"), value: null },
]);

const visibleColumnKeys = computed(() => new Set(props.viewOptions.visibleColumns));
const visibleColumnsOrdered = computed(() =>
  columnOptions.value.filter((column) => visibleColumnKeys.value.has(column.key)),
);

const filteredDownloads = computed(() => props.downloads);

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

const {
  currentPage,
  pageSize,
  totalPages,
  paginatedItems: pagedDownloads,
  pageStart,
  pageEnd,
  goToPreviousPage,
  goToNextPage,
} = usePagination(sortedDownloads, 20);

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
      priorityMenu.value = null;
    }
  },
);

watch(contextMenu, (menu) => {
  if (menu) {
    nextTick(() => {
      const firstItem = contextMenuPanelRef.value?.querySelector<HTMLButtonElement>(
        ".task-context-menu__item:not(:disabled)",
      );
      firstItem?.focus();
    });
  }
});

function handlePriorityMenuKeydown(event: KeyboardEvent) {
  if (!priorityMenu.value) return;

  switch (event.key) {
    case "Escape":
      event.preventDefault();
      closePriorityMenu();
      break;
    case "ArrowDown":
    case "ArrowUp": {
      event.preventDefault();
      const items = Array.from(
        priorityMenuRef.value?.querySelectorAll<HTMLButtonElement>(".priority-menu__item") ?? [],
      );
      if (items.length === 0) return;
      const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = (currentIndex + direction + items.length) % items.length;
      items[nextIndex]?.focus();
      break;
    }
  }
}

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

function isColumnVisible(key: ColumnKey) {
  return visibleColumnKeys.value.has(key);
}

function handleRowKeydown(event: KeyboardEvent, downloadId: string) {
  if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
    event.preventDefault();
    emit("select", downloadId);
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const { x, y } = clampMenuPosition(rect.left + rect.width / 2, rect.top + rect.height / 2);
    backgroundMenu.value = null;
    contextMenu.value = { downloadId, x, y };
  }
}

function closeAllMenus() {
  contextMenu.value = null;
  priorityMenu.value = null;
  backgroundMenu.value = null;
}

function closeBackgroundMenu() {
  backgroundMenu.value = null;
}

const showContextMenu = computed(() => contextMenu.value !== null);
useFloatingClose(contextMenuPanelRef, showContextMenu, closeAllMenus);

const showBackgroundMenu = computed(() => backgroundMenu.value !== null);
useFloatingClose(backgroundMenuPanelRef, showBackgroundMenu, closeBackgroundMenu);

function handleMenuKeydown(event: KeyboardEvent, panel: HTMLElement | null) {
  if (!panel) return;

  switch (event.key) {
    case "Escape":
      event.preventDefault();
      closeAllMenus();
      break;
    case "ArrowDown":
    case "ArrowUp": {
      event.preventDefault();
      const items = Array.from(
        panel.querySelectorAll<HTMLButtonElement>(".task-context-menu__item:not(:disabled)"),
      );
      if (items.length === 0) return;
      const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = (currentIndex + direction + items.length) % items.length;
      items[nextIndex]?.focus();
      break;
    }
  }
}

function clampMenuPosition(clientX: number, clientY: number) {
  const menuWidth = 220;
  const menuHeight = 360;
  const gutter = 12;

  return {
    x: Math.max(gutter, Math.min(clientX, window.innerWidth - menuWidth - gutter)),
    y: Math.max(gutter, Math.min(clientY, window.innerHeight - menuHeight - gutter)),
  };
}

function openTaskContextMenu(event: MouseEvent, downloadId: string) {
  emit("select", downloadId);

  backgroundMenu.value = null;
  const { x, y } = clampMenuPosition(event.clientX, event.clientY);
  contextMenu.value = { downloadId, x, y };
}

function openBackgroundContextMenu(event: MouseEvent) {
  contextMenu.value = null;
  priorityMenu.value = null;

  const { x, y } = clampMenuPosition(event.clientX, event.clientY);
  backgroundMenu.value = { x, y };
}

function handleNewTask() {
  backgroundMenu.value = null;
  emit("newTask");
}

function handleRefresh() {
  backgroundMenu.value = null;
  emit("refresh");
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

function isFlushing(download: DownloadSummary) {
  return download.flushing && download.state === "downloading";
}

function progressBarValue(download: DownloadSummary) {
  return isFlushing(download) ? 100 : progressValue(download);
}

function progressPrimaryText(download: DownloadSummary) {
  return isFlushing(download) ? t("queue.flushing") : progressLabel(download);
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
</script>

<template>
  <section class="queue-panel grid gap-4" @contextmenu.prevent="openBackgroundContextMenu($event)">
    <div class="desk-panel__header queue-panel__header py-1">
      <div>
        <p class="section-kicker">{{ t("queue.kicker") }}</p>
        <h2 class="panel-title">{{ t("queue.title") }}</h2>
      </div>

      <div class="queue-panel__actions inline-flex items-center gap-2 flex-wrap">
        <span
          class="sync-pill inline-flex items-center justify-center min-h-7 min-w-[6.5rem] px-3 rounded-md border text-xs tracking-normal capitalize"
          :data-active="isSyncIndicatorVisible"
          >{{ isSyncIndicatorVisible ? t("queue.autoSyncing") : t("queue.idle") }}</span
        >
      </div>
    </div>

    <div v-if="sortedDownloads.length" class="queue-panel__table grid gap-[0.625rem]">
      <div class="queue-table-shell min-h-[30rem] border rounded-md overflow-hidden">
        <table
          class="queue-table w-full border-collapse table-fixed"
          :class="{ 'queue-table--compact': viewOptions.compactView }"
        >
          <thead>
            <tr>
              <th
                v-if="multiSelect.multiSelectMode"
                class="queue-cell--checkbox w-10 text-center align-middle p-0"
              >
                <input
                  type="checkbox"
                  class="w-4 h-4 m-0 cursor-pointer"
                  :checked="allPageSelected"
                  @change="toggleSelectAllOnPage"
                />
              </th>
              <th
                v-for="column in visibleColumnsOrdered"
                :key="column.key"
                class="h-8 px-3 text-left text-xs font-semibold uppercase tracking-wider"
              >
                {{ column.label }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="download in pagedDownloads"
              :key="download.id"
              :data-testid="`download-row-${download.id}`"
              class="queue-row min-h-[3.5rem] cursor-pointer"
              :class="{
                'queue-row--active': !multiSelect.multiSelectMode && download.id === selectedId,
                'queue-row--selected':
                  multiSelect.multiSelectMode && multiSelect.selectedIds.has(download.id),
              }"
              @click="
                multiSelect.multiSelectMode
                  ? $emit('toggleSelect', download.id)
                  : $emit('select', download.id)
              "
              @dblclick="$emit('doubleClick', download.id)"
              @contextmenu.prevent.stop="openTaskContextMenu($event, download.id)"
              @keydown="handleRowKeydown($event, download.id)"
              tabindex="0"
            >
              <td
                v-if="multiSelect.multiSelectMode"
                class="queue-cell w-10 text-center align-middle p-0"
              >
                <input
                  type="checkbox"
                  class="w-4 h-4 m-0 cursor-pointer"
                  :checked="multiSelect.selectedIds.has(download.id)"
                  @change="$emit('toggleSelect', download.id)"
                />
              </td>
              <td
                v-if="isColumnVisible('file')"
                class="queue-cell queue-cell--file w-[24%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                <div class="queue-file grid gap-[0.1rem] min-w-0">
                  <span class="queue-file__title flex items-center gap-[0.4rem] min-w-0">
                    <span
                      class="queue-file__name min-w-0 font-semibold text-[0.82rem] leading-[1.2] truncate"
                      >{{ download.fileName }}</span
                    >
                    <span
                      class="queue-file__kind flex-none px-[0.3rem] py-[0.05rem] rounded-sm border text-[0.6rem] font-semibold leading-[1.3]"
                      >{{ labelForTaskKind(download.kind) }}</span
                    >
                    <UiBadge
                      v-if="download.cdnAccelerated"
                      size="sm"
                      tone="warning"
                      class="queue-file__cdn flex-none inline-flex items-center gap-[0.15rem] text-[0.6rem] font-semibold"
                    >
                      <span class="i-ri-flashlight-fill" aria-hidden="true" />
                      {{ download.cdnNodeIp || "CDN" }}
                    </UiBadge>
                    <UiBadge
                      v-if="download.degraded"
                      size="sm"
                      tone="warning"
                      class="queue-file__degraded flex-none inline-flex items-center gap-[0.15rem] text-[0.6rem] font-semibold cursor-help"
                      :title="t('queue.degradedHint')"
                    >
                      {{ t("queue.degraded") }}
                    </UiBadge>
                    <UiBadge
                      v-if="download.diskType === 'hdd'"
                      size="sm"
                      tone="info"
                      class="queue-file__hdd flex-none inline-flex items-center gap-[0.15rem] text-[0.6rem] font-semibold cursor-help"
                      :title="t('queue.hddHint')"
                    >
                      <span class="i-ri-hard-drive-2-line" aria-hidden="true" />
                      HDD
                    </UiBadge>
                  </span>
                  <span class="queue-file__path text-[0.72rem] leading-[1.2] font-mono truncate">{{
                    download.destinationPath
                  }}</span>
                  <span class="queue-file__meta text-[0.68rem] leading-[1.2] truncate">{{
                    metaForDownload(download)
                  }}</span>
                </div>
              </td>

              <td
                v-if="isColumnVisible('size')"
                class="queue-cell queue-cell--size w-[8%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{ formatBytes(download.totalBytes) }}
              </td>

              <td
                v-if="isColumnVisible('downloaded')"
                class="queue-cell queue-cell--downloaded w-[10%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{ formatBytes(download.downloadedBytes) }}
              </td>

              <td
                v-if="isColumnVisible('status')"
                class="queue-cell queue-cell--status w-[10%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                <UiBadge
                  data-testid="task-status"
                  size="sm"
                  :tone="isFlushing(download) ? 'info' : toneForState(download.state)"
                  >{{
                    isFlushing(download) ? t("queue.flushingShort") : t(`states.${download.state}`)
                  }}</UiBadge
                >
              </td>

              <td
                v-if="isColumnVisible('progress')"
                class="queue-cell queue-cell--progress w-[18%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                <div class="queue-progress grid gap-[0.2rem]">
                  <div class="queue-progress__copy flex justify-between gap-2 text-[0.7rem]">
                    <span
                      data-testid="task-progress-label"
                      aria-live="polite"
                      aria-atomic="false"
                      :class="{ 'queue-progress__flushing': isFlushing(download) }"
                      >{{ progressPrimaryText(download) }}</span
                    >
                    <span>
                      {{ formatBytes(download.downloadedBytes) }} /
                      {{ formatBytes(download.totalBytes) }}
                    </span>
                  </div>
                  <UiProgress
                    :value="progressBarValue(download)"
                    :indeterminate="
                      isSizeUnknown(download) &&
                      download.state !== 'completed' &&
                      !isFlushing(download)
                    "
                  />
                </div>
              </td>

              <td
                v-if="isColumnVisible('speed')"
                data-testid="task-speed"
                class="queue-cell queue-cell--speed w-[10%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{ formatSpeed(download.speedBytesPerSecond) }}
              </td>

              <td
                v-if="isColumnVisible('priority')"
                class="queue-cell queue-cell--priority w-[8%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                <button
                  type="button"
                  class="queue-cell__priority inline-flex items-center gap-[0.2rem] cursor-pointer"
                  :title="t('composer.priority')"
                  @click.stop="openPriorityMenu($event, download.id)"
                >
                  <UiBadge size="sm" :tone="priorityTone(download.priority)">
                    {{ priorityLabel(download.priority) }}
                  </UiBadge>
                  <span class="i-ri-arrow-down-s-line text-[0.65rem]" aria-hidden="true" />
                </button>
              </td>

              <td
                v-if="isColumnVisible('uploadSpeed')"
                class="queue-cell queue-cell--up-speed w-[8%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{
                  download.kind === "bt"
                    ? formatSpeed(download.uploadSpeedBytesPerSecond)
                    : "\u2014"
                }}
              </td>

              <td
                v-if="isColumnVisible('seeds')"
                class="queue-cell queue-cell--seeds w-[6%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{
                  download.kind === "bt"
                    ? `${download.seedCount ?? "\u2014"}/${download.leechCount ?? "\u2014"}`
                    : "\u2014"
                }}
              </td>

              <td
                v-if="isColumnVisible('eta')"
                class="queue-cell queue-cell--eta w-[6%] px-2 py-1 align-middle text-[0.8125rem]"
              >
                {{ formatEta(download.etaSeconds) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="queue-pagination flex items-center justify-between gap-3 flex-wrap">
        <p class="queue-pagination__summary m-0 text-sm">
          {{
            t("queue.showing", { start: pageStart, end: pageEnd, total: sortedDownloads.length })
          }}
        </p>
        <div class="inline-flex items-center gap-2 flex-wrap">
          <span class="text-sm whitespace-nowrap">{{ t("queue.pageSize") }}</span>
          <UiSelect
            :model-value="pageSize"
            :options="pageSizeOptions"
            :aria-label="t('queue.rowsPerPage')"
            class="w-24"
            @update:model-value="pageSize = $event"
          />
          <div class="queue-pagination__actions inline-flex items-center gap-[0.35rem] flex-wrap">
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
            <span class="queue-pagination__page m-0 text-sm">{{
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
      </div>

      <Teleport to="body">
        <div
          v-if="contextMenu && contextMenuDownload"
          ref="contextMenuPanelRef"
          class="task-context-menu fixed z-30 min-w-48 grid gap-[0.15rem] p-[0.35rem] border rounded-md"
          :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
          @pointerdown.stop
          @keydown="handleMenuKeydown($event, contextMenuPanelRef)"
        >
          <button
            type="button"
            class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            :disabled="!canTogglePauseOrResume"
            @click="handlePauseOrResume"
          >
            <span :class="contextActionIcon" aria-hidden="true" />
            <span>{{ contextActionLabel }}</span>
          </button>
          <template v-if="contextMenuDownload?.kind === 'bt'">
            <button
              type="button"
              class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
              @click="onSetBtSpeedLimit"
            >
              <span class="i-ri-speed-up-line" aria-hidden="true" />
              <span>{{ t("queue.setSpeedLimit") }}</span>
            </button>
          </template>
          <hr class="task-context-menu__divider" />
          <div class="task-context-menu__group">
            <span
              class="task-context-menu__group-label px-[0.6rem] text-[0.68rem] uppercase tracking-wider"
            >
              {{ t("composer.priority") }}
            </span>
            <button
              v-for="option in priorityOptions"
              :key="option.value"
              type="button"
              class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
              :class="{
                'task-context-menu__item--active': contextMenuDownload?.priority === option.value,
              }"
              @click="handleSetPriority(contextMenuDownload!.id, option.value)"
            >
              <span
                class="w-2 h-2 rounded-full priority-menu__dot"
                :class="`priority-menu__dot--${option.value}`"
                aria-hidden="true"
              />
              <span>{{ option.label }}</span>
              <span
                v-if="contextMenuDownload?.priority === option.value"
                class="i-ri-check-line ml-auto"
                aria-hidden="true"
              />
            </button>
          </div>
          <hr class="task-context-menu__divider" />
          <button
            type="button"
            class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            @click="handleDeleteTask"
          >
            <span class="i-ri-delete-bin-6-line" aria-hidden="true" />
            <span>{{ t("queue.deleteTask") }}</span>
          </button>
          <button
            type="button"
            class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            @click="handleCopyLink"
          >
            <span class="i-ri-file-copy-line" aria-hidden="true" />
            <span>{{ t("queue.copyLink") }}</span>
          </button>
          <button
            type="button"
            class="task-context-menu__item task-context-menu__item--danger flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            @click="handleDeleteTaskPermanently"
          >
            <span class="i-ri-delete-bin-line" aria-hidden="true" />
            <span>{{ t("queue.permanentDelete") }}</span>
          </button>
          <button
            type="button"
            class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            @click="handleOpenInExplorer"
          >
            <span class="i-ri-folder-open-line" aria-hidden="true" />
            <span>{{ t("queue.openInExplorer") }}</span>
          </button>
        </div>
      </Teleport>

      <Teleport to="body">
        <div
          v-if="priorityMenu && priorityMenuRef"
          ref="priorityMenuRef"
          class="priority-menu fixed z-30 min-w-[7.5rem] grid gap-[0.15rem] p-[0.35rem] border rounded-md"
          :style="{ left: `${priorityMenu.x}px`, top: `${priorityMenu.y}px` }"
          @pointerdown.stop
          @keydown="handlePriorityMenuKeydown"
        >
          <button
            v-for="option in priorityOptions"
            :key="option.value"
            type="button"
            class="priority-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
            :class="{
              'priority-menu__item--active':
                priorityMenu?.downloadId &&
                downloads.find((d) => d.id === priorityMenu?.downloadId)?.priority === option.value,
            }"
            @click="
              priorityMenu?.downloadId && handleSetPriority(priorityMenu.downloadId, option.value)
            "
          >
            <span
              class="priority-menu__dot w-2 h-2 rounded-full"
              :class="`priority-menu__dot--${option.value}`"
              aria-hidden="true"
            />
            <span>{{ option.label }}</span>
            <span
              v-if="
                priorityMenu?.downloadId &&
                downloads.find((d) => d.id === priorityMenu?.downloadId)?.priority === option.value
              "
              class="i-ri-check-line ml-auto"
              aria-hidden="true"
            />
          </button>
        </div>
      </Teleport>
    </div>

    <UiEmptyState
      v-else-if="downloads.length"
      icon="i-ri-search-eye-line"
      :title="t('queue.noResults')"
    />
    <UiEmptyState
      v-else
      icon="i-ri-inbox-archive-line"
      :title="t('queue.emptyTitle')"
      :description="t('queue.emptyDescription')"
    />

    <Teleport to="body">
      <div
        v-if="backgroundMenu"
        ref="backgroundMenuPanelRef"
        class="task-context-menu fixed z-30 min-w-48 grid gap-[0.15rem] p-[0.35rem] border rounded-md"
        :style="{ left: `${backgroundMenu.x}px`, top: `${backgroundMenu.y}px` }"
        role="menu"
        :aria-label="t('queue.title')"
        @pointerdown.stop
        @keydown="handleMenuKeydown($event, backgroundMenuPanelRef)"
      >
        <button
          type="button"
          role="menuitem"
          class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
          @click="handleNewTask"
        >
          <span class="i-ri-add-line" aria-hidden="true" />
          <span>{{ t("nav.newTask") }}</span>
        </button>
        <button
          type="button"
          role="menuitem"
          class="task-context-menu__item flex items-center gap-[0.6rem] min-h-8 px-[0.6rem] border-0 rounded-sm bg-transparent text-sm text-left cursor-pointer"
          @click="handleRefresh"
        >
          <span class="i-ri-refresh-line" aria-hidden="true" />
          <span>{{ t("common.refresh") }}</span>
        </button>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.sync-pill {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  color: var(--color-text-muted);
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

.queue-table-shell {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
}

.queue-table thead th {
  border-bottom: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  color: var(--color-text-muted);
  letter-spacing: var(--letter-spacing-wide);
}

.queue-table tbody tr {
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

.queue-cell input {
  accent-color: var(--color-accent);
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

.queue-cell--size {
  color: var(--color-text-muted);
}

.queue-cell--downloaded {
  color: var(--color-text-soft);
}

.queue-cell--speed {
  color: var(--color-text-muted);
}

.queue-cell--up-speed {
  color: var(--color-text-muted);
}

.queue-cell--seeds {
  color: var(--color-text-muted);
}

.queue-cell--priority {
  color: var(--color-text-main);
}

.queue-cell__priority {
  appearance: none;
  border: none;
  background: transparent;
  padding: 0;
  color: inherit;
}

.queue-cell__priority:focus-visible {
  outline: none;
  border-radius: var(--radius-sm);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.queue-cell--eta {
  color: var(--color-text-muted);
}

.queue-file__name {
  color: var(--color-heading);
}

.queue-file__kind {
  border: 1px solid var(--color-border);
  background: var(--color-panel-muted);
  color: var(--color-text-muted);
}

.queue-file__cdn .i-ri-flashlight-fill,
.queue-file__hdd .i-ri-hard-drive-2-line {
  font-size: 0.7rem;
}

.queue-file__path {
  color: var(--color-text-muted);
}

.queue-file__meta {
  color: var(--color-text-soft);
}

.queue-progress__copy {
  color: var(--color-text-muted);
}

.queue-progress__flushing {
  color: var(--color-info-text);
  animation: queue-flush-pulse 1.6s ease-in-out infinite;
}

@keyframes queue-flush-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.55;
  }
}

@media (prefers-reduced-motion: reduce) {
  .queue-progress__flushing {
    animation: none;
  }
}

.queue-empty {
  color: var(--color-text-muted);
  border: 1px dashed var(--color-border-strong);
  background: var(--color-panel-muted);
}

.queue-empty__icon {
  color: var(--color-accent);
}

.queue-pagination__summary,
.queue-pagination__page {
  color: var(--color-text-muted);
}

.task-context-menu {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  box-shadow: var(--shadow-card-hover);
}

.task-context-menu__item {
  color: var(--color-text-main);
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

.task-context-menu__item--active {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
}

.task-context-menu__item--active:hover:not(:disabled) {
  background: var(--color-accent-soft);
}

.task-context-menu__divider {
  height: 1px;
  border: none;
  margin: 0.15rem 0.35rem;
  background: var(--color-border);
}

.task-context-menu__group-label {
  display: block;
  color: var(--color-text-muted);
  font-weight: 600;
  letter-spacing: var(--letter-spacing-wide);
  padding: 0.25rem 0.6rem;
}

.priority-menu {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  box-shadow: var(--shadow-card-hover);
}

.priority-menu__item {
  color: var(--color-text-main);
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.priority-menu__item:hover:not(:disabled) {
  background: var(--color-surface-muted);
}

.priority-menu__item--active {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
}

.priority-menu__item--active:hover:not(:disabled) {
  background: var(--color-accent-soft);
}

.priority-menu__dot {
  flex: none;
}

.priority-menu__dot--high {
  background: var(--color-danger-text);
}

.priority-menu__dot--normal {
  background: var(--color-text-muted);
}

.priority-menu__dot--low {
  background: var(--color-info-text);
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
