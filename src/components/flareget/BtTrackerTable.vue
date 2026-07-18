<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../i18n";
import type { BtTrackerInfo } from "../../types/download";
import DataTable from "../ui/DataTable.vue";
import type { DataTableColumn } from "../ui/DataTable.vue";

const props = defineProps<{ trackers: BtTrackerInfo[] }>();

const { t } = useI18n();

const columns: DataTableColumn[] = [
  { key: "url", label: t("inspector.trackerTable.url") },
];

const rows = computed(() =>
  props.trackers.map((tracker) => ({
    url: tracker.url,
  })),
);
</script>

<template>
  <DataTable
    :columns="columns"
    :rows="rows"
    :empty-title="t('inspector.trackerTable.empty')"
  />
</template>

<style scoped>
/* All styling delegated to DataTable component */
</style>
