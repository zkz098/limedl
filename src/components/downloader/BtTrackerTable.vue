<script setup lang="ts">
import { useI18n } from "../../i18n";
import type { BtTrackerInfo } from "../../types/download";

defineProps<{ trackers: BtTrackerInfo[] }>();

const { t } = useI18n();
</script>

<template>
  <div v-if="trackers.length === 0" class="bt-tracker-table__empty">
    {{ t("inspector.trackerTable.empty") }}
  </div>
  <table v-else class="bt-tracker-table">
    <thead>
      <tr>
        <th>{{ t("inspector.trackerTable.url") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="(tracker, i) in trackers" :key="i">
        <td class="bt-tracker-table__url" :title="tracker.url">
          {{ tracker.url }}
        </td>
      </tr>
    </tbody>
  </table>
</template>

<style scoped>
.bt-tracker-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.bt-tracker-table thead th {
  height: 2.25rem;
  padding: 0 0.75rem;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-surface-muted);
  color: var(--color-text-soft);
  font-size: 0.74rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-align: left;
  text-transform: uppercase;
}

.bt-tracker-table tbody tr {
  height: 2.5rem;
  transition: background-color 0.2s ease;
}

.bt-tracker-table tbody tr + tr td {
  border-top: 1px solid var(--color-border);
}

.bt-tracker-table tbody tr:hover {
  background: color-mix(in srgb, var(--color-accent-soft) 28%, var(--color-panel));
}

.bt-tracker-table__url {
  padding: 0.3rem 0.75rem;
  vertical-align: middle;
  font-family: "Cascadia Code", "Fira Code", "Consolas", monospace;
  font-size: 0.8rem;
  color: var(--color-text);
  max-width: 0;
  width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bt-tracker-table__empty {
  padding: 1.5rem 0.75rem;
  color: var(--color-text-soft);
  font-size: 0.84rem;
  text-align: center;
}
</style>
