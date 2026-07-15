<script setup lang="ts">
import { computed } from "vue";

import { formatBytes, formatSpeed } from "../../lib/download-format";
import { t } from "../../i18n";
import type { BtRuntimeStatus } from "../../types/download";
import StatRow from "./StatRow.vue";

const props = defineProps<{
  status: BtRuntimeStatus | null;
}>();

const isConnected = computed(() => Boolean(props.status?.connected));
const statusLabel = computed(() =>
  isConnected.value ? t("sidebar.btConnected") : t("sidebar.btDisconnected"),
);
const dhtNodesLabel = computed(() => {
  if (!props.status?.dhtEnabled) return t("common.disabled");
  return typeof props.status.dhtNodes === "number"
    ? String(props.status.dhtNodes)
    : t("common.unknown");
});
const uploadSpeedLabel = computed(() => formatSpeed(props.status?.uploadSpeedBytesPerSecond));
const uploadedLabel = computed(() => formatBytes(props.status?.uploadedBytes));
const peerCountLabel = computed(() => String(props.status?.peerCount ?? 0));
const torrentCountLabel = computed(() => String(props.status?.torrentCount ?? 0));
const seedCountLabel = computed(() =>
  props.status?.seedCount == null ? "—" : String(props.status.seedCount),
);
const leechCountLabel = computed(() =>
  props.status?.leechCount == null ? "—" : String(props.status.leechCount),
);
</script>

<template>
  <section class="sidebar-bt" :class="{ 'sidebar-bt--connected': isConnected }">
    <div class="sidebar-bt__heading">
      <p class="section-kicker">{{ t("sidebar.btStatus") }}</p>
      <span class="sidebar-bt__badge">
        <span class="sidebar-bt__dot" aria-hidden="true" />
        {{ statusLabel }}
      </span>
    </div>

    <div class="sidebar-bt__grid">
      <StatRow :label="t('sidebar.btDhtNodes')" :value="dhtNodesLabel" />
      <StatRow :label="t('sidebar.btUploadSpeed')" :value="uploadSpeedLabel" />
      <StatRow :label="t('sidebar.btPeers')" :value="peerCountLabel" />
      <StatRow :label="t('sidebar.btTorrents')" :value="torrentCountLabel" />
      <StatRow :label="t('sidebar.btUploaded')" :value="uploadedLabel" />
      <StatRow :label="t('sidebar.btSeeds')" :value="seedCountLabel" />
      <StatRow :label="t('sidebar.btLeeches')" :value="leechCountLabel" />
    </div>
  </section>
</template>

<style scoped>
.sidebar-bt {
  display: grid;
  gap: 0.55rem;
  padding: 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
}

.sidebar-bt__heading {
  display: grid;
  gap: 0.35rem;
}

.sidebar-bt__badge {
  width: fit-content;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  color: var(--color-text-muted);
  font-size: 0.75rem;
  font-weight: 600;
}

.sidebar-bt__dot {
  width: 0.45rem;
  height: 0.45rem;
  border-radius: var(--radius-pill);
  background: var(--color-text-muted);
}

.sidebar-bt--connected .sidebar-bt__badge {
  color: var(--color-accent-strong);
}

.sidebar-bt--connected .sidebar-bt__dot {
  background: var(--color-accent);
}

.sidebar-bt__grid {
  display: grid;
  gap: 0.35rem;
}
</style>
