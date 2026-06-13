<script setup lang="ts">
import { computed } from "vue";

import { formatBytes } from "../../lib/download-format";
import type { ChunkInfo } from "../../types/download";

const props = defineProps<{
  chunks: ChunkInfo[];
  title: string;
  totalBytes: number;
}>();

const MAX_SQUARES = 64;

interface ChunkGroup {
  startIndex: number;
  endIndex: number;
  downloaded: number;
  total: number;
  completed: boolean;
  active: boolean;
  progress: number;
}

const groups = computed<ChunkGroup[]>(() => {
  if (!props.chunks?.length) {
    return [];
  }

  const raw = props.chunks;
  if (raw.length <= MAX_SQUARES) {
    return raw.map((chunk) => {
      const total = Math.max(0, chunk.end - chunk.start + 1);
      const progress = total > 0 ? chunk.downloaded / total : 0;
      return {
        startIndex: chunk.index,
        endIndex: chunk.index,
        downloaded: chunk.downloaded,
        total,
        completed: chunk.completed,
        active: chunk.claimedBy !== null,
        progress: Math.min(progress, 1),
      };
    });
  }

  const groupSize = Math.ceil(raw.length / MAX_SQUARES);
  const result: ChunkGroup[] = [];

  for (let i = 0; i < raw.length; i += groupSize) {
    const slice = raw.slice(i, i + groupSize);
    let downloaded = 0;
    let total = 0;
    let completed = true;
    let active = false;

    for (const chunk of slice) {
      const chunkTotal = Math.max(0, chunk.end - chunk.start + 1);
      downloaded += chunk.downloaded;
      total += chunkTotal;
      if (!chunk.completed) {
        completed = false;
      }
      if (chunk.claimedBy !== null) {
        active = true;
      }
    }

    const progress = total > 0 ? downloaded / total : 0;
    result.push({
      startIndex: slice[0].index,
      endIndex: slice[slice.length - 1].index,
      downloaded,
      total,
      completed,
      active,
      progress: Math.min(progress, 1),
    });
  }

  return result;
});

function tooltipText(group: ChunkGroup): string {
  const percent = (group.progress * 100).toFixed(0);
  if (group.startIndex === group.endIndex) {
    return `Chunk #${group.startIndex}: ${percent}% (${formatBytes(group.downloaded)}/${formatBytes(group.total)})`;
  }
  return `Chunks #${group.startIndex}-${group.endIndex}: ${percent}% (${formatBytes(group.downloaded)}/${formatBytes(group.total)})`;
}
</script>

<template>
  <section v-if="groups.length" class="chunk-heatmap">
    <h3 class="chunk-heatmap__title">{{ title }}</h3>
    <div class="chunk-heatmap__grid">
      <div
        v-for="group in groups"
        :key="group.startIndex"
        class="chunk-heatmap__cell"
        :class="{
          'chunk-heatmap__cell--completed': group.completed,
          'chunk-heatmap__cell--active': group.active && !group.completed,
        }"
        :style="{ '--cell-opacity': String(Math.max(0.15, group.progress)) }"
        :title="tooltipText(group)"
      />
    </div>
  </section>
</template>

<style scoped>
.chunk-heatmap {
  display: grid;
  gap: var(--space-2);
}

.chunk-heatmap__title {
  margin: 0;
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--color-text-muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.chunk-heatmap__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(1.25rem, 1fr));
  gap: 0.25rem;
}

.chunk-heatmap__cell {
  position: relative;
  aspect-ratio: 1;
  border-radius: var(--radius-sm);
  background-color: var(--color-border);
  transition: transform var(--duration-fast) ease;
}

.chunk-heatmap__cell--completed {
  background-color: var(--color-accent);
}

.chunk-heatmap__cell--active::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background-color: var(--color-accent);
  opacity: var(--cell-opacity);
}

.chunk-heatmap__cell:hover {
  transform: scale(1.15);
  z-index: 1;
}
</style>
