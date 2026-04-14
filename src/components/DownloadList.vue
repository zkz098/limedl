<script setup lang="ts">
import { watch } from "vue";
import { usePolling } from "../composables/usePolling";
import { useDownloader } from "../composables/useDownloader";
import type { DownloadSummary } from "../types/download";
import DataTable from "primevue/datatable";
import Column from "primevue/column";
import Button from "primevue/button";
import ProgressBar from "primevue/progressbar";

const { downloads, selectedId, selectDownload, refreshList } = useDownloader();
const { isPolling, start } = usePolling(() => refreshList(), 2000);

watch(
  () => true,
  () => start(),
  { once: true }
);

function formatBytes(value?: number): string {
  if (typeof value !== "number") return "—";
  if (value === 0) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let index = 0;

  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }

  const precision = size >= 100 || index === 0 ? 0 : 1;
  return `${size.toFixed(precision)} ${units[index]}`;
}

function formatSpeed(value?: number): string {
  if (typeof value !== "number") return "—";
  return `${formatBytes(value)}/s`;
}

function formatEta(value?: number): string {
  if (typeof value !== "number") return "—";

  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const seconds = value % 60;
  const parts = [
    hours ? `${hours}h` : "",
    minutes ? `${minutes}m` : "",
    `${seconds}s`,
  ].filter(Boolean);

  return parts.join(" ");
}

function progressPercent(download: DownloadSummary): number {
  if (!download.totalBytes || download.totalBytes <= 0) return 0;
  return Math.min((download.downloadedBytes / download.totalBytes) * 100, 100);
}

function getStatusClass(state: string): string {
  const classes: Record<string, string> = {
    queued: "bg-blue-100 text-blue-800",
    downloading: "bg-green-100 text-green-800",
    paused: "bg-yellow-100 text-yellow-800",
    completed: "bg-emerald-100 text-emerald-800",
    failed: "bg-red-100 text-red-800",
    canceled: "bg-red-100 text-red-800",
  };
  return classes[state] || "bg-gray-100 text-gray-800";
}
</script>

<template>
  <div class="download-list-container">
    <DataTable
      :value="downloads"
      :rows="10"
      paginator
      responsive-layout="scroll"
      :loading="isPolling"
      data-key="id"
      show-gridlines
      striped-rows
      size="small"
      scroll-height="flex"
      class="p-datatable-sm"
    >
      <Column field="fileName" header="File" style="width: 25%">
        <template #body="{ data }">
          <div class="truncate font-semibold">{{ data.fileName }}</div>
        </template>
      </Column>

      <Column field="state" header="Status" style="width: 12%">
        <template #body="{ data }">
          <span
            class="px-2 py-1 rounded text-xs font-semibold"
            :class="getStatusClass(data.state)"
          >
            {{ data.state.toUpperCase() }}
          </span>
        </template>
      </Column>

      <Column header="Progress" style="width: 20%">
        <template #body="{ data }">
          <div class="space-y-1">
            <ProgressBar :value="progressPercent(data)" :show-value="true" />
            <div class="text-xs text-gray-500">
              {{ formatBytes(data.downloadedBytes) }} / {{ formatBytes(data.totalBytes) }}
            </div>
          </div>
        </template>
      </Column>

      <Column field="speedBytesPerSecond" header="Speed" style="width: 15%">
        <template #body="{ data }">
          {{ formatSpeed(data.speedBytesPerSecond) }}
        </template>
      </Column>

      <Column field="etaSeconds" header="ETA" style="width: 12%">
        <template #body="{ data }">
          {{ formatEta(data.etaSeconds) }}
        </template>
      </Column>

      <Column header="Action" style="width: 16%">
        <template #body="{ data }">
          <Button
            :label="selectedId === data.id ? 'Selected' : 'Select'"
            :severity="selectedId === data.id ? 'success' : 'info'"
            size="small"
            @click="selectDownload(data.id)"
          />
        </template>
      </Column>

      <template #empty>
        <div class="text-center py-8 text-gray-500">
          No downloads yet. Start by adding a new download URL.
        </div>
      </template>
    </DataTable>
  </div>
</template>

<style scoped>
.download-list-container {
  width: 100%;
  height: 100%;
}

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
