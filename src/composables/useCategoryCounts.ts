import { computed, type Ref, type ComputedRef } from "vue";
import type { DownloadSummary } from "../types/download";
import { formatSpeed } from "../lib/download-format";

export interface CategoryCounts {
  "": number;
  downloading: number;
  paused: number;
  completed: number;
  failed: number;
  active: number;
}

export interface SidebarStats {
  totalTasks: number;
  activeTasks: number;
  completedTasks: number;
  currentSpeed: string;
}

export function useCategoryCounts(downloads: Ref<DownloadSummary[]>): {
  categoryCounts: ComputedRef<CategoryCounts>;
  sidebarStats: ComputedRef<SidebarStats>;
} {
  const categoryCounts = computed<CategoryCounts>(() => {
    const counts: CategoryCounts = {
      "": 0,
      downloading: 0,
      paused: 0,
      completed: 0,
      failed: 0,
      active: 0,
    };
    for (const d of downloads.value) {
      counts[""]++;
      const s = d.state;
      if (s === "downloading" || s === "queued") {
        counts.downloading++;
        counts.active++;
      } else if (s === "paused") {
        counts.paused++;
        counts.active++;
      } else if (s === "completed") {
        counts.completed++;
      } else if (s === "failed" || s === "canceled") {
        counts.failed++;
      }
    }
    return counts;
  });

  const sidebarStats = computed<SidebarStats>(() => ({
    totalTasks: downloads.value.length,
    activeTasks: categoryCounts.value.active,
    completedTasks: categoryCounts.value.completed,
    currentSpeed: formatSpeed(
      downloads.value
        .filter((d) => d.state === "downloading")
        .reduce((sum, d) => sum + (d.speedBytesPerSecond ?? 0), 0),
    ),
  }));

  return { categoryCounts, sidebarStats };
}
