<script setup lang="ts">
import { computed } from "vue";

import { formatBytes, formatSpeed } from "../../lib/download-format";
import { t } from "../../i18n";
import type { BtRuntimeStatus } from "../../types/download";

const props = defineProps<{
  status: BtRuntimeStatus | null;
}>();

const isConnected = computed(() => Boolean(props.status?.connected));
const statusLabel = computed(() =>
  isConnected.value ? t("sidebar.btConnected") : t("sidebar.btDisconnected"),
);
const dhtNodesLabel = computed(() => {
  if (!props.status?.dhtEnabled) {
    return t("common.disabled");
  }

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
      <p>
        <span>{{ t("sidebar.btDhtNodes") }}</span>
        <strong>{{ dhtNodesLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btUploadSpeed") }}</span>
        <strong>{{ uploadSpeedLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btPeers") }}</span>
        <strong>{{ peerCountLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btTorrents") }}</span>
        <strong>{{ torrentCountLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btUploaded") }}</span>
        <strong>{{ uploadedLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btSeeds") }}</span>
        <strong>{{ seedCountLabel }}</strong>
      </p>
      <p>
        <span>{{ t("sidebar.btLeeches") }}</span>
        <strong>{{ leechCountLabel }}</strong>
      </p>
    </div>
  </section>
</template>

<style scoped>
.sidebar-bt {
  display: grid;
  gap: 0.65rem;
  padding: 0.85rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) var(--surface-muted-alpha), transparent);
  backdrop-filter: blur(var(--surface-blur));
}

.sidebar-bt__heading {
  display: grid;
  gap: 0.45rem;
}

.sidebar-bt__badge {
  width: fit-content;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  color: var(--color-text-muted);
  font-size: 0.78rem;
  font-weight: 600;
}

.sidebar-bt__dot {
  width: 0.48rem;
  height: 0.48rem;
  border-radius: 999px;
  background: var(--color-text-muted);
}

.sidebar-bt--connected .sidebar-bt__badge {
  color: var(--color-accent-strong);
}

.sidebar-bt--connected .sidebar-bt__dot {
  background: var(--color-accent);
  box-shadow: 0 0 0 0.18rem color-mix(in srgb, var(--color-accent) 16%, transparent);
}

.sidebar-bt__grid {
  display: grid;
  gap: 0.4rem;
}

.sidebar-bt__grid p {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  color: var(--color-text-muted);
  font-size: 0.78rem;
  line-height: 1.4;
}

.sidebar-bt__grid strong {
  min-width: 0;
  color: var(--color-heading);
  font-size: 0.78rem;
  font-weight: 600;
  text-align: right;
  overflow-wrap: anywhere;
}
</style>
