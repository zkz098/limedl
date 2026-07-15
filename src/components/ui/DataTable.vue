<script setup lang="ts">
import UiEmptyState from "./UiEmptyState.vue";

export interface DataTableColumn {
  key: string;
  label: string;
  width?: string;
  align?: "left" | "right" | "center";
}

defineProps<{
  columns: DataTableColumn[];
  rows: Array<Record<string, string>>;
  emptyTitle?: string;
  emptyIcon?: string;
  rowKey?: string;
}>();
</script>

<template>
  <template v-if="rows.length === 0">
    <UiEmptyState
      :title="emptyTitle ?? ''"
      :icon="emptyIcon"
    />
  </template>
  <table v-else class="data-table">
    <thead>
      <tr>
        <th
          v-for="col in columns"
          :key="col.key"
          class="data-table__th"
          :style="col.width ? { width: col.width } : undefined"
          :class="col.align ? `data-table__th--${col.align}` : ''"
        >
          {{ col.label }}
        </th>
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="(row, rowIndex) in rows"
        :key="rowKey && row[rowKey] !== undefined ? row[rowKey] : rowIndex"
        class="data-table__row"
      >
        <td
          v-for="col in columns"
          :key="col.key"
          class="data-table__cell"
          :style="col.width ? { width: col.width } : undefined"
          :class="col.align ? `data-table__cell--${col.align}` : ''"
        >
          {{ row[col.key] ?? "" }}
        </td>
      </tr>
    </tbody>
  </table>
</template>

<style scoped>
.data-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.data-table__th {
  height: 2.25rem;
  padding: 0 0.75rem;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-muted);
  font-size: 0.74rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--color-text-muted);
  text-align: left;
}

.data-table__th--right {
  text-align: right;
}

.data-table__th--center {
  text-align: center;
}

.data-table__cell {
  padding: 0.3rem 0.75rem;
  vertical-align: middle;
}

.data-table__cell--right {
  text-align: right;
}

.data-table__cell--center {
  text-align: center;
}

.data-table__row + .data-table__row .data-table__cell {
  border-top: 1px solid var(--color-border);
}

.data-table__row:hover {
  background: var(--color-surface-muted);
}
</style>
