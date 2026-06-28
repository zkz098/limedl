<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../i18n";
import type { BtPieceInfo } from "../../types/download";

const props = defineProps<{
  pieces: BtPieceInfo[];
  title: string;
}>();

const { t } = useI18n();

const MAX_SQUARES = 64;

interface CellInfo {
  index: number;
  completed: boolean;
  title: string;
}

const cells = computed<CellInfo[]>(() => {
  if (!props.pieces?.length) {
    return [];
  }

  if (props.pieces.length <= MAX_SQUARES) {
    return props.pieces.map((p) => ({
      index: p.index,
      completed: p.completed,
      title: t("inspector.pieceHeatmap.tooltip", { index: p.index }),
    }));
  }

  const groupSize = Math.ceil(props.pieces.length / MAX_SQUARES);
  const result: CellInfo[] = [];
  for (let i = 0; i < MAX_SQUARES; i++) {
    const start = i * groupSize;
    const end = Math.min(start + groupSize, props.pieces.length);
    const group = props.pieces.slice(start, end);
    const allCompleted = group.length > 0 && group.every((p) => p.completed);
    const completedCount = group.filter((p) => p.completed).length;
    result.push({
      index: start,
      completed: allCompleted,
      title: `${completedCount}/${group.length} — pieces #${start}–#${end - 1}`,
    });
  }
  return result;
});
</script>

<template>
  <section v-if="cells.length" class="bt-piece-heatmap">
    <h3 class="bt-piece-heatmap__title">{{ title }}</h3>
    <div class="bt-piece-heatmap__grid">
      <div
        v-for="cell in cells"
        :key="cell.index"
        class="bt-piece-heatmap__cell"
        :class="{ 'bt-piece-heatmap__cell--completed': cell.completed }"
        :title="cell.title"
      />
    </div>
  </section>
</template>

<style scoped>
.bt-piece-heatmap {
  display: grid;
  gap: var(--space-2);
}

.bt-piece-heatmap__title {
  margin: 0;
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--color-text-muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.bt-piece-heatmap__grid {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 0.25rem;
}

.bt-piece-heatmap__cell {
  position: relative;
  aspect-ratio: 1;
  border-radius: var(--radius-sm);
  background-color: var(--color-border);
  transition: transform var(--duration-fast) ease;
}

.bt-piece-heatmap__cell--completed {
  background-color: var(--color-accent);
}

.bt-piece-heatmap__cell:hover {
  transform: scale(1.15);
  z-index: 1;
}
</style>
