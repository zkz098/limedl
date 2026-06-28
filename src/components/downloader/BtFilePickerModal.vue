<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { TorrentFileEntry } from "../../types/download";
import { formatBytes } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import UiDialog from "../ui/UiDialog.vue";
import UiButton from "../ui/UiButton.vue";
import UiSelect from "../ui/UiSelect.vue";

const props = defineProps<{
  modelValue: boolean;
  files: TorrentFileEntry[];
  isPreviewing: boolean;
  previewError: string | null;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [selections: { index: number; priority: number }[]];
  cancel: [];
  retry: [];
}>();

const { t } = useI18n();

// Priority values.
// NOTE: librqbit's AddTorrentOptions.only_files is a flat index list (all-or-nothing per file).
// Low / Normal / High distinction is cosmetic and does NOT affect the actual download.
// Only Skip (priority 0) maps to "excluded from selectedFileIndices".
const PRIORITY_SKIP = 0;
const PRIORITY_LOW = 1;
const PRIORITY_NORMAL = 4;
const PRIORITY_HIGH = 7;

const priorityOptions = [
  { label: t("filePicker.prioritySkip"), value: PRIORITY_SKIP },
  { label: t("filePicker.priorityLow"), value: PRIORITY_LOW },
  { label: t("filePicker.priorityNormal"), value: PRIORITY_NORMAL },
  { label: t("filePicker.priorityHigh"), value: PRIORITY_HIGH },
];

interface FileSelection {
  selected: boolean;
  priority: number;
}

const fileSelections = ref<Map<number, FileSelection>>(new Map());

function buildSelections(files: TorrentFileEntry[]): Map<number, FileSelection> {
  const map = new Map<number, FileSelection>();
  for (const f of files) {
    const existing = fileSelections.value.get(f.index);
    map.set(f.index, existing ?? { selected: true, priority: PRIORITY_NORMAL });
  }
  return map;
}

// TODO L5: buildSelections replaces fileSelections entirely on files change, discarding user priority edits; preserve existing selections across updates
watch(
  () => props.files,
  (files) => {
    fileSelections.value = buildSelections(files);
  },
  { immediate: true },
);

// ---------------
// Selection toggles
// ---------------

function selectAll() {
  const next = new Map(fileSelections.value);
  for (const [, sel] of next) {
    sel.selected = true;
    if (sel.priority === PRIORITY_SKIP) {
      sel.priority = PRIORITY_NORMAL;
    }
  }
  fileSelections.value = next;
}

function deselectAll() {
  const next = new Map(fileSelections.value);
  for (const [, sel] of next) {
    sel.selected = false;
    sel.priority = PRIORITY_SKIP;
  }
  fileSelections.value = next;
}

function toggleSelection(index: number) {
  const sel = fileSelections.value.get(index);
  if (!sel) return;
  const next = new Map(fileSelections.value);
  const entry = next.get(index)!;
  entry.selected = !entry.selected;
  if (!entry.selected) {
    entry.priority = PRIORITY_SKIP;
  } else if (entry.priority === PRIORITY_SKIP) {
    entry.priority = PRIORITY_NORMAL;
  }
  fileSelections.value = next;
}

function setPriority(index: number, priority: number) {
  const sel = fileSelections.value.get(index);
  if (!sel) return;
  const next = new Map(fileSelections.value);
  const entry = next.get(index)!;
  entry.priority = priority;
  if (priority === PRIORITY_SKIP) {
    entry.selected = false;
  }
  fileSelections.value = next;
}

// ---------------
// Computed
// ---------------

const selectedCount = computed(() => {
  let count = 0;
  for (const sel of fileSelections.value.values()) {
    if (sel.selected) count += 1;
  }
  return count;
});

const hasSelection = computed(() => selectedCount.value > 0);

// ---------------
// Actions
// ---------------

function confirmDownload() {
  const selections: { index: number; priority: number }[] = [];
  for (const [index, sel] of fileSelections.value) {
    if (sel.selected && sel.priority > PRIORITY_SKIP) {
      selections.push({ index, priority: sel.priority });
    }
  }
  emit("confirm", selections);
}

function onCancel() {
  emit("cancel");
  emit("update:modelValue", false);
}

// ---------------
// Display helpers
// ---------------

function pathDepth(path: string): number {
  return Math.min(path.split("/").length - 1, 5);
}

function rowClass(index: number) {
  const sel = fileSelections.value.get(index);
  return {
    "bt-file-picker__row--dimmed": sel ? !sel.selected : false,
  };
}
</script>

<template>
  <UiDialog
    :model-value="modelValue"
    :close-on-overlay="false"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template #title>
      {{ t("filePicker.title") }}
    </template>

    <!-- Loading state -->
    <div v-if="isPreviewing && files.length === 0 && !previewError" class="bt-file-picker__loading">
      <span class="i-ri-loader-4-line bt-file-picker__spinner" aria-hidden="true" />
      <p>{{ t("filePicker.loading") }}</p>
    </div>

    <!-- Error state -->
    <div v-else-if="previewError" class="bt-file-picker__loading">
      <p class="bt-file-picker__error-text">{{ t("filePicker.error") }}</p>
      <p class="bt-file-picker__error-detail">{{ previewError }}</p>
      <UiButton variant="ghost" size="sm" icon="i-ri-refresh-line" @click="emit('retry')">
        {{ t("common.refresh") }}
      </UiButton>
    </div>

    <!-- Initial / empty (no files yet, not loading, no error) -->
    <div v-else-if="files.length === 0" class="bt-file-picker__loading">
      <p>{{ t("filePicker.loading") }}</p>
    </div>

    <!-- File list -->
    <div v-else class="bt-file-picker__body">
      <div class="bt-file-picker__toolbar">
        <UiButton variant="ghost" size="sm" @click="selectAll">
          {{ t("filePicker.selectAll") }}
        </UiButton>
        <UiButton variant="ghost" size="sm" @click="deselectAll">
          {{ t("filePicker.deselectAll") }}
        </UiButton>
      </div>

      <!-- Column header row -->
      <div class="bt-file-picker__header">
        <span class="bt-file-picker__col-check" />
        <span class="bt-file-picker__col-path">{{ t("filePicker.path") }}</span>
        <span class="bt-file-picker__col-size">{{ t("filePicker.size") }}</span>
        <span class="bt-file-picker__col-priority">{{ t("filePicker.priority") }}</span>
      </div>

      <ul class="bt-file-picker__list">
        <li
          v-for="file in files"
          :key="file.index"
          class="bt-file-picker__row"
          :class="rowClass(file.index)"
        >
          <div class="bt-file-picker__col-check">
            <input
              type="checkbox"
              :checked="fileSelections.get(file.index)?.selected ?? false"
              @change="toggleSelection(file.index)"
            />
          </div>
          <div
            class="bt-file-picker__col-path"
            :style="{ paddingLeft: `calc(var(--space-2) * ${pathDepth(file.path)})` }"
          >
            <span class="bt-file-picker__path-text">{{ file.path }}</span>
          </div>
          <div class="bt-file-picker__col-size">
            {{ formatBytes(file.size) }}
          </div>
          <div class="bt-file-picker__col-priority">
            <UiSelect
              :model-value="fileSelections.get(file.index)?.priority ?? PRIORITY_NORMAL"
              :options="priorityOptions"
              @update:model-value="setPriority(file.index, $event as number)"
            />
          </div>
        </li>
      </ul>

      <div class="bt-file-picker__footer">
        <span class="bt-file-picker__selected-count">
          {{ selectedCount }} / {{ files.length }} {{ t("filePicker.confirm") }}
        </span>
        <div class="bt-file-picker__footer-actions">
          <UiButton variant="secondary" @click="onCancel">
            {{ t("filePicker.cancel") }}
          </UiButton>
          <UiButton variant="primary" :disabled="!hasSelection" @click="confirmDownload">
            {{ t("filePicker.confirm") }}
          </UiButton>
        </div>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.bt-file-picker__loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-8) 0;
  color: var(--color-text-muted);
}

.bt-file-picker__spinner {
  font-size: 2rem;
  animation: spin 1s linear infinite;
}

.bt-file-picker__error-text {
  color: var(--color-danger-text);
  font-weight: 600;
}

.bt-file-picker__error-detail {
  font-size: 0.875rem;
  color: var(--color-text-dim);
  max-width: 32rem;
  text-align: center;
}

.bt-file-picker__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.bt-file-picker__toolbar {
  display: flex;
  gap: var(--space-2);
  padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--color-border);
}

.bt-file-picker__header {
  display: grid;
  grid-template-columns: 2.25rem 1fr 6rem 10rem;
  gap: var(--space-2);
  align-items: center;
  padding: 0 var(--space-2);
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--color-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.bt-file-picker__list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 18rem;
  overflow-y: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
}

.bt-file-picker__row {
  display: grid;
  grid-template-columns: 2.25rem 1fr 6rem 10rem;
  gap: var(--space-2);
  align-items: center;
  padding: var(--space-2);
  border-bottom: 1px solid var(--color-border-subtle);
  transition: opacity 0.2s ease;
}

.bt-file-picker__row:last-child {
  border-bottom: none;
}

.bt-file-picker__row--dimmed {
  opacity: 0.4;
}

.bt-file-picker__col-check {
  display: flex;
  align-items: center;
  justify-content: center;
}

.bt-file-picker__col-check input[type="checkbox"] {
  width: 1rem;
  height: 1rem;
  accent-color: var(--color-accent);
  cursor: pointer;
}

.bt-file-picker__col-path {
  min-width: 0;
}

.bt-file-picker__path-text {
  font-family: var(--font-mono);
  font-size: 0.8625rem;
  word-break: break-all;
  color: var(--color-text-main);
}

.bt-file-picker__col-size {
  font-size: 0.85rem;
  color: var(--color-text-dim);
  text-align: right;
  white-space: nowrap;
}

.bt-file-picker__col-priority {
  min-width: 0;
}

.bt-file-picker__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border);
}

.bt-file-picker__selected-count {
  font-size: 0.85rem;
  color: var(--color-text-dim);
}

.bt-file-picker__footer-actions {
  display: flex;
  gap: var(--space-2);
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
