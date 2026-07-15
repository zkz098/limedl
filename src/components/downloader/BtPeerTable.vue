<script setup lang="ts">
import { computed } from "vue";
import { formatSpeed } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import type { BtPeerInfo } from "../../types/download";
import DataTable from "../ui/DataTable.vue";
import type { DataTableColumn } from "../ui/DataTable.vue";

const { t } = useI18n();

const props = defineProps<{
  peers: BtPeerInfo[];
}>();

const columns: DataTableColumn[] = [
  { key: "ip", label: t("inspector.peerTable.ip"), width: "22%" },
  { key: "client", label: t("inspector.peerTable.client"), width: "22%" },
  { key: "flags", label: t("inspector.peerTable.flags"), width: "16%" },
  { key: "dlSpeed", label: t("inspector.peerTable.dlSpeed"), width: "14%" },
  { key: "ulSpeed", label: t("inspector.peerTable.ulSpeed"), width: "14%" },
  { key: "progress", label: t("inspector.peerTable.progress"), width: "12%" },
];

const rows = computed(() =>
  props.peers.map((peer) => ({
    ip: peer.address,
    client: peer.client || "\u2014",
    flags: peer.flags || "\u2014",
    dlSpeed: formatSpeed(peer.downloadSpeed),
    ulSpeed: formatSpeed(peer.uploadSpeed),
    progress: `${Math.round(peer.progress * 100)}%`,
  })),
);
</script>

<template>
  <DataTable
    :columns="columns"
    :rows="rows"
    :empty-title="t('inspector.peerTable.empty')"
  />
</template>

<style scoped>
/* All styling delegated to DataTable component */
</style>
