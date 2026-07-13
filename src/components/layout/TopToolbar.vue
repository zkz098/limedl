<script setup lang="ts">
import { t } from "../../i18n";
import { formatSpeed } from "../../lib/download-format";
import UiButton from "../ui/UiButton.vue";

defineProps<{
  searchQuery: string;
  hasSelection: boolean;
  btStatus: {
    dhtNodes: number;
    uploadSpeed: number;
    peers: number;
    torrents: number;
  } | null;
}>();

const emit = defineEmits<{
  "update:searchQuery": [value: string];
  "add-task": [];
  delete: [];
  refresh: [];
}>();

function handleSearchInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emit("update:searchQuery", target.value);
}

function handleSearchClear() {
  emit("update:searchQuery", "");
}
</script>

<template>
  <div class="top-toolbar">
    <div class="toolbar-actions">
      <UiButton
        variant="primary"
        size="sm"
        icon="i-ri-add-line"
        @click="emit('add-task')"
      >
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
      <UiButton
        variant="ghost"
        size="sm"
        icon="i-ri-refresh-line"
        @click="emit('refresh')"
      >
        {{ t("toolbar.refresh") }}
      </UiButton>
    </div>

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
