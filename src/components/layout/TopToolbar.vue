<script setup lang="ts">
import { computed, ref } from "vue";

import { t } from "../../i18n";
import { formatSpeed } from "../../lib/download-format";
import type { ColumnKey } from "../../lib/column-defs";
import { VALID_COLUMN_KEYS } from "../../lib/column-defs";
import { useFloatingClose } from "../../composables/useFloatingClose";
import type { SortDirection, SortKey } from "../../types/settings";
import UiButton from "../ui/UiButton.vue";
import UiSelect from "../ui/UiSelect.vue";

const props = defineProps<{
  searchQuery: string;
  hasSelection: boolean;
  btStatus: {
    dhtNodes: number;
    uploadSpeed: number;
    peers: number;
    torrents: number;
  } | null;
  sortKey: SortKey;
  sortDirection: SortDirection;
  compactView: boolean;
  visibleColumns: string[];
  multiSelectMode: boolean;
  selectedCount: number;
  filteredCount: number;
  gameMode?: boolean;
  overclockMode?: boolean;
}>();

const emit = defineEmits<{
  "update:searchQuery": [value: string];
  "update:sortKey": [value: SortKey];
  "update:sortDirection": [value: SortDirection];
  "update:compactView": [value: boolean];
  "update:visibleColumns": [value: string[]];
  "add-task": [];
  delete: [];
  refresh: [];
  "update:multiSelectMode": [value: boolean];
  pauseAll: [];
  resumeAll: [];
  clearCompleted: [];
  selectAll: [];
  deselectAll: [];
  batchDelete: [];
  toggleGameMode: [];
  toggleOverclockMode: [];
}>();

const columnMenuOpen = ref(false);
const columnMenuPanelRef = ref<HTMLDivElement | null>(null);
useFloatingClose(columnMenuPanelRef, columnMenuOpen, closeColumnMenu);

const sortOptions = computed<Array<{ value: SortKey; label: string }>>(() => [
  { value: "name", label: t("toolbar.sortBy.name") },
  { value: "size", label: t("toolbar.sortBy.size") },
  { value: "progress", label: t("toolbar.sortBy.progress") },
  { value: "speed", label: t("toolbar.sortBy.speed") },
  { value: "added_at", label: t("toolbar.sortBy.addedAt") },
  { value: "state", label: t("toolbar.sortBy.state") },
]);

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

const sortDirectionIcon = computed(() =>
  props.sortDirection === "asc" ? "i-ri-arrow-up-line" : "i-ri-arrow-down-line",
);

function handleSearchInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emit("update:searchQuery", target.value);
}

function handleSearchClear() {
  emit("update:searchQuery", "");
}

function handleSortKeyChange(value: SortKey) {
  emit("update:sortKey", value);
}

const allSelected = computed(
  () => props.selectedCount > 0 && props.selectedCount >= props.filteredCount,
);

const selectAllIcon = computed(() =>
  allSelected.value ? "i-ri-checkbox-blank-line" : "i-ri-checkbox-multiple-line",
);

function handleSelectToggle() {
  if (allSelected.value) {
    emit("deselectAll");
  } else {
    emit("selectAll");
  }
}

function toggleSortDirection() {
  emit("update:sortDirection", props.sortDirection === "asc" ? "desc" : "asc");
}

function toggleCompactView() {
  emit("update:compactView", !props.compactView);
}

function isColumnVisible(key: string) {
  return props.visibleColumns.includes(key);
}

function toggleColumn(key: string) {
  const option = columnOptions.value.find((item) => item.key === key);
  if (option?.alwaysVisible) {
    return;
  }

  const nextColumns = isColumnVisible(key)
    ? props.visibleColumns.filter((column) => column !== key)
    : [...props.visibleColumns, key];

  emit(
    "update:visibleColumns",
    columnOptions.value.map((item) => item.key).filter((column) => nextColumns.includes(column)),
  );
}

function closeColumnMenu() {
  columnMenuOpen.value = false;
}
</script>

<template>
  <div class="top-toolbar flex items-center flex-wrap gap-2 px-4 py-2 min-h-0">
    <div class="toolbar-actions flex items-center gap-1">
      <UiButton variant="primary" size="sm" icon="i-ri-add-line" @click="emit('add-task')">
        {{ t("toolbar.addTask") }}
      </UiButton>
      <UiButton
        variant="ghost"
        size="sm"
        icon="i-ri-delete-bin-line"
        :disabled="!hasSelection"
        @click="emit('delete')"
      >
        {{ t("toolbar.delete") }}
      </UiButton>
      <UiButton variant="ghost" size="sm" icon="i-ri-refresh-line" @click="emit('refresh')">
        {{ t("toolbar.refresh") }}
      </UiButton>
    </div>

    <div class="toolbar-divider w-px h-5 flex-shrink-0" />

    <!-- Multi-select mode toggle -->
    <UiButton
      variant="ghost"
      size="sm"
      icon="i-ri-checkbox-multiple-line"
      :class="{ 'toolbar-btn--active': multiSelectMode }"
      @click="$emit('update:multiSelectMode', !multiSelectMode)"
    >
      {{ t("toolbar.multiSelectMode") }}
    </UiButton>

    <template v-if="multiSelectMode">
      <div class="toolbar-divider w-px h-5 flex-shrink-0" />
      <div class="toolbar-batch-actions flex items-center gap-1">
        <span
          v-if="selectedCount > 0"
          class="toolbar-selected-count text-sm font-semibold whitespace-nowrap px-1"
        >
          {{ t("toolbar.selectedCount", { count: selectedCount }) }}
        </span>
        <UiButton variant="ghost" size="sm" icon="i-ri-pause-line" @click="$emit('pauseAll')">
          {{ t("toolbar.pauseAll") }}
        </UiButton>
        <UiButton variant="ghost" size="sm" icon="i-ri-play-line" @click="$emit('resumeAll')">
          {{ t("toolbar.resumeAll") }}
        </UiButton>
        <UiButton
          variant="ghost"
          size="sm"
          icon="i-ri-check-double-line"
          @click="$emit('clearCompleted')"
        >
          {{ t("toolbar.clearCompleted") }}
        </UiButton>
        <UiButton variant="ghost" size="sm" :icon="selectAllIcon" @click="handleSelectToggle">
          {{ allSelected ? t("toolbar.deselectAll") : t("toolbar.selectAll") }}
        </UiButton>
        <UiButton
          variant="ghost"
          size="sm"
          icon="i-ri-delete-bin-line"
          :disabled="selectedCount === 0"
          @click="$emit('batchDelete')"
        >
          {{ t("toolbar.batchDelete") }}
        </UiButton>
      </div>
    </template>

    <div class="toolbar-divider w-px h-5 flex-shrink-0" />

    <div class="toolbar-view-controls flex items-center gap-1 flex-shrink-0">
      <div class="sort-control flex items-center gap-1">
        <UiSelect
          :model-value="sortKey"
          :options="sortOptions"
          :aria-label="t('toolbar.sortBy.label')"
          class="sort-control__select"
          @update:model-value="handleSortKeyChange"
        />
        <UiButton
          size="sm"
          variant="ghost"
          :icon="sortDirectionIcon"
          @click="toggleSortDirection"
        />
      </div>

      <UiButton
        size="sm"
        :variant="compactView ? 'secondary' : 'ghost'"
        icon="i-ri-list-check"
        :title="t('toolbar.compactView')"
        @click="toggleCompactView"
      />

      <div class="column-menu relative">
        <UiButton
          size="sm"
          variant="ghost"
          icon="i-ri-settings-3-line"
          :title="t('toolbar.columns')"
          @click.stop="columnMenuOpen = !columnMenuOpen"
        >
          {{ t("toolbar.columns") }}
        </UiButton>
        <div
          v-if="columnMenuOpen"
          ref="columnMenuPanelRef"
          class="column-menu__panel absolute top-[calc(100%+0.35rem)] right-0 z-5 min-w-36 grid gap-[0.15rem] p-[0.35rem] border rounded-md"
        >
          <label
            v-for="column in columnOptions"
            :key="column.key"
            class="column-menu__item flex items-center justify-between gap-2 p-[0.35rem_0.45rem] rounded-sm border border-transparent text-sm cursor-pointer"
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
              class="column-menu__indicator w-4 inline-flex justify-center text-sm"
              :class="isColumnVisible(column.key) ? 'i-ri-check-line' : 'i-ri-add-line'"
              aria-hidden="true"
            />
            <span class="flex-1">{{ column.label }}</span>
          </label>
        </div>
      </div>
    </div>

    <div class="toolbar-divider w-px h-5 flex-shrink-0" />

    <div class="toolbar-spacer flex-1" />

    <div class="toolbar-search relative w-64">
      <span
        class="toolbar-search__icon i-ri-search-line absolute left-[0.625rem] top-1/2 -translate-y-1/2 text-sm pointer-events-none"
        aria-hidden="true"
      />
      <input
        :value="searchQuery"
        type="text"
        class="toolbar-search__input w-full border rounded-md py-1 pl-8 pr-3 font-inherit text-sm outline-none"
        :placeholder="t('toolbar.searchPlaceholder')"
        @input="handleSearchInput"
      />
      <button
        v-if="searchQuery"
        type="button"
        class="toolbar-search__clear absolute right-[0.375rem] top-1/2 -translate-y-1/2 flex items-center justify-center w-5 h-5 border-none rounded-sm bg-transparent cursor-pointer text-xs p-0"
        :aria-label="t('queue.clearSearch')"
        @click="handleSearchClear"
      >
        <span class="i-ri-close-line" aria-hidden="true" />
      </button>
    </div>

    <div
      v-if="btStatus"
      class="toolbar-bt flex items-center gap-2 flex-shrink-0"
      data-testid="toolbar-bt-status"
    >
      <span
        class="toolbar-bt__pill inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs leading-none whitespace-nowrap"
        data-testid="toolbar-bt-dht-count"
        :title="t('toolbar.dhtNodes')"
      >
        <span class="i-ri-global-line" aria-hidden="true" />
        <span>{{ btStatus.dhtNodes }}</span>
      </span>
      <span
        class="toolbar-bt__pill inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs leading-none whitespace-nowrap"
        data-testid="toolbar-bt-upload-speed"
        :title="t('toolbar.uploadSpeed')"
      >
        <span class="i-ri-upload-2-line" aria-hidden="true" />
        <span>{{ formatSpeed(btStatus.uploadSpeed) }}</span>
      </span>
      <span
        class="toolbar-bt__pill inline-flex items-center gap-1 rounded-full px-2 py-1 text-xs leading-none whitespace-nowrap"
        :title="t('toolbar.peers')"
      >
        <span class="i-ri-users-line" aria-hidden="true" />
        <span>{{ btStatus.peers }}</span>
      </span>
    </div>

    <button
      type="button"
      class="game-mode-btn inline-flex items-center justify-center min-w-[1.875rem] min-h-[1.875rem] px-[0.45rem] border rounded-md cursor-pointer"
      :class="{ 'game-mode-btn--active': gameMode }"
      :title="gameMode ? t('toolbar.gameModeActive') : t('toolbar.gameModeInactive')"
      @click="$emit('toggleGameMode')"
    >
      <span
        class="game-mode-btn__icon text-base"
        :class="gameMode ? 'i-ri-gamepad-fill' : 'i-ri-gamepad-line'"
        aria-hidden="true"
      />
    </button>

    <button
      type="button"
      class="overclock-btn inline-flex items-center justify-center min-w-[1.875rem] min-h-[1.875rem] px-[0.45rem] border rounded-md cursor-pointer"
      :class="{ 'overclock-btn--active': overclockMode }"
      :title="overclockMode ? t('toolbar.overclockActive') : t('toolbar.overclockInactive')"
      @click="$emit('toggleOverclockMode')"
    >
      <span
        class="overclock-btn__icon text-base"
        :class="overclockMode ? 'i-ri-flashlight-fill' : 'i-ri-flashlight-line'"
        aria-hidden="true"
      />
    </button>
  </div>
</template>

<style scoped>
.top-toolbar {
  background: var(--color-panel);
  border-bottom: 1px solid var(--color-border);
}

.toolbar-btn--active {
  background: var(--color-accent-soft);
  border-color: var(--color-accent-soft-border);
  color: var(--color-accent-strong);
}

.toolbar-selected-count {
  color: var(--color-accent-strong);
}

.toolbar-divider {
  background: var(--color-border);
}

.sort-control__select {
  width: auto;
  min-width: 6rem;
}

.column-menu__panel {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  box-shadow: var(--shadow-card);
}

.column-menu__item {
  color: var(--color-text-main);
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

.column-menu__item input {
  width: 0.9rem;
  height: 0.9rem;
  accent-color: var(--color-accent);
}

.toolbar-search__icon {
  color: var(--color-text-muted);
}

.toolbar-search__input {
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  color: var(--color-text-main);
  font: inherit;
  transition: border-color 0.15s ease;
}

.toolbar-search__input::placeholder {
  color: var(--color-text-soft);
}

.toolbar-search__input:focus {
  border-color: var(--color-accent-border);
}

.toolbar-search__input:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.toolbar-search__clear {
  color: var(--color-text-muted);
}

.toolbar-search__clear:hover {
  background: var(--color-border-strong);
  color: var(--color-text-main);
}

.toolbar-bt__pill {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
}

.overclock-btn {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  color: var(--color-text-muted);
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    color 0.2s ease;
}

.overclock-btn:hover {
  border-color: var(--color-border-strong);
  background: var(--color-panel-muted);
}

.overclock-btn--active {
  border-color: var(--color-accent-soft-border);
  background: var(--color-accent-soft);
  color: var(--color-accent);
}

.game-mode-btn {
  border: 1px solid var(--color-border);
  background: var(--color-panel);
  color: var(--color-text-muted);
  transition:
    background-color 0.2s ease,
    border-color 0.2s ease,
    color 0.2s ease;
}

.game-mode-btn:hover {
  border-color: var(--color-border-strong);
  background: var(--color-panel-muted);
}

.game-mode-btn--active {
  border-color: var(--color-accent-soft-border);
  background: var(--color-accent-soft);
  color: var(--color-accent);
}
</style>
