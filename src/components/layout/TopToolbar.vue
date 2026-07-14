<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";

import { t } from "../../i18n";
import { formatSpeed } from "../../lib/download-format";
import type { ColumnKey } from "../../lib/column-defs";
import { VALID_COLUMN_KEYS } from "../../lib/column-defs";
import type { SortDirection, SortKey } from "../../types/settings";
import UiButton from "../ui/UiButton.vue";

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
}>();

const columnMenuOpen = ref(false);
const columnMenuButtonRef = ref<HTMLButtonElement | null>(null);

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

function handleSortKeyChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value as SortKey;
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

function handleGlobalPointerDown(event: PointerEvent) {
  if (!columnMenuOpen.value) {
    return;
  }

  const target = event.target as Node;
  if (columnMenuButtonRef.value?.contains(target)) {
    return;
  }

  closeColumnMenu();
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeColumnMenu();
  }
}

onMounted(() => {
  window.addEventListener("pointerdown", handleGlobalPointerDown);
  window.addEventListener("keydown", handleEscape);
});

onUnmounted(() => {
  window.removeEventListener("pointerdown", handleGlobalPointerDown);
  window.removeEventListener("keydown", handleEscape);
});
</script>

<template>
  <div class="top-toolbar">
    <div class="toolbar-actions">
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

    <div class="toolbar-divider" />

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
      <div class="toolbar-divider" />
      <div class="toolbar-batch-actions">
        <span v-if="selectedCount > 0" class="toolbar-selected-count">
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

    <div class="toolbar-divider" />

    <div class="toolbar-view-controls">
      <div class="sort-control">
        <div class="sort-control__select">
          <select
            :value="sortKey"
            class="sort-control__native"
            :aria-label="t('toolbar.sortBy.label')"
            @change="handleSortKeyChange"
          >
            <option v-for="option in sortOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
          <span class="i-ri-arrow-down-s-line sort-control__arrow" aria-hidden="true" />
        </div>
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

      <div class="column-menu">
        <UiButton
          ref="columnMenuButtonRef"
          size="sm"
          variant="ghost"
          icon="i-ri-settings-3-line"
          :title="t('toolbar.columns')"
          @click.stop="columnMenuOpen = !columnMenuOpen"
        >
          {{ t("toolbar.columns") }}
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
    </div>

    <div class="toolbar-divider" />

    <div class="toolbar-spacer" />

    <div class="toolbar-search">
      <span class="toolbar-search__icon i-ri-search-line" aria-hidden="true" />
      <input
        :value="searchQuery"
        type="text"
        class="toolbar-search__input"
        :placeholder="t('toolbar.searchPlaceholder')"
        @input="handleSearchInput"
      />
      <button
        v-if="searchQuery"
        type="button"
        class="toolbar-search__clear"
        aria-label="Clear search"
        @click="handleSearchClear"
      >
        <span class="i-ri-close-line" aria-hidden="true" />
      </button>
    </div>

    <div v-if="btStatus" class="toolbar-bt">
      <span class="toolbar-bt__pill" :title="t('toolbar.dhtNodes')">
        <span class="i-ri-global-line" aria-hidden="true" />
        <span>{{ btStatus.dhtNodes }}</span>
      </span>
      <span class="toolbar-bt__pill" :title="t('toolbar.uploadSpeed')">
        <span class="i-ri-upload-2-line" aria-hidden="true" />
        <span>{{ formatSpeed(btStatus.uploadSpeed) }}</span>
      </span>
      <span class="toolbar-bt__pill" :title="t('toolbar.peers')">
        <span class="i-ri-users-line" aria-hidden="true" />
        <span>{{ btStatus.peers }}</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.top-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  background: var(--color-panel);
  border-bottom: 1px solid var(--color-border);
  min-height: 0;
}

/* ── Actions ── */

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.toolbar-btn--active {
  background: var(--color-accent-soft);
  border-color: var(--color-accent-soft-border);
  color: var(--color-accent-strong);
}

/* ── Batch actions ── */

.toolbar-batch-actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.toolbar-selected-count {
  color: var(--color-accent-strong);
  font-size: var(--font-size-small);
  font-weight: 600;
  white-space: nowrap;
  padding: 0 var(--space-1);
}

/* ── Dividers ── */

.toolbar-divider {
  width: 1px;
  height: 1.25rem;
  background: var(--color-border);
  flex-shrink: 0;
}

/* ── View controls ── */

.toolbar-view-controls {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex-shrink: 0;
}

.sort-control {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.sort-control__select {
  position: relative;
}

.sort-control__native {
  appearance: none;
  min-height: 1.875rem;
  padding: 0 1.75rem 0 0.625rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  font-size: var(--font-size-small);
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.sort-control__native:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.sort-control__arrow {
  position: absolute;
  right: 0.5rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-muted);
  pointer-events: none;
  font-size: 0.875rem;
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

/* ── Spacer ── */

.toolbar-spacer {
  flex: 1;
}

/* ── Search ── */

.toolbar-search {
  position: relative;
  width: 16rem;
}

.toolbar-search__icon {
  position: absolute;
  left: 0.625rem;
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.875rem;
  color: var(--color-text-muted);
  pointer-events: none;
}

.toolbar-search__input {
  width: 100%;
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--space-1) var(--space-3) var(--space-1) 2rem;
  font: inherit;
  font-size: 0.8125rem;
  color: var(--color-text-main);
  outline: none;
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
  position: absolute;
  right: 0.375rem;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.25rem;
  height: 1.25rem;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 0;
}

.toolbar-search__clear:hover {
  background: var(--color-border-strong);
  color: var(--color-text-main);
}

/* ── BT status ── */

.toolbar-bt {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.toolbar-bt__pill {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  background: var(--color-surface-muted);
  border-radius: var(--radius-pill);
  padding: var(--space-1) var(--space-2);
  color: var(--color-text-muted);
  font-size: 0.75rem;
  line-height: 1;
  white-space: nowrap;
}
</style>
