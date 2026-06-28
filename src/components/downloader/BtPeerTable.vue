<script setup lang="ts">
import { formatSpeed } from "../../lib/download-format";
import { useI18n } from "../../i18n";
import type { BtPeerInfo } from "../../types/download";
import UiBadge from "../ui/UiBadge.vue";
import UiProgress from "../ui/UiProgress.vue";

const { t } = useI18n();

defineProps<{
  peers: BtPeerInfo[];
}>();

function formatPeerSpeed(bytesPerSec: number): string {
  return formatSpeed(bytesPerSec);
}

function peerProgress(peer: BtPeerInfo): number {
  // progress is a 0.0–1.0 fraction from the backend
  return Math.round(peer.progress * 100);
}
</script>

<template>
  <div class="bt-peer-table">
    <div v-if="peers.length === 0" class="bt-peer-table__empty">
      <p>{{ t("inspector.peerTable.empty") }}</p>
    </div>

    <div v-else class="bt-peer-table__shell">
      <table class="bt-peer-table__table">
        <thead>
          <tr>
            <th class="bt-peer-table__th bt-peer-table__col--ip">
              {{ t("inspector.peerTable.ip") }}
            </th>
            <th class="bt-peer-table__th bt-peer-table__col--client">
              {{ t("inspector.peerTable.client") }}
            </th>
            <th class="bt-peer-table__th bt-peer-table__col--flags">
              {{ t("inspector.peerTable.flags") }}
            </th>
            <th class="bt-peer-table__th bt-peer-table__col--dl-speed">
              {{ t("inspector.peerTable.dlSpeed") }}
            </th>
            <th class="bt-peer-table__th bt-peer-table__col--ul-speed">
              {{ t("inspector.peerTable.ulSpeed") }}
            </th>
            <th class="bt-peer-table__th bt-peer-table__col--progress">
              {{ t("inspector.peerTable.progress") }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(peer, index) in peers"
            :key="`${peer.address}-${index}`"
            class="bt-peer-table__row"
          >
            <td class="bt-peer-table__cell bt-peer-table__cell--address">
              {{ peer.address }}
            </td>
            <td class="bt-peer-table__cell bt-peer-table__cell--client">
              <span v-if="peer.client" class="bt-peer-table__client-name">
                {{ peer.client }}
              </span>
              <span v-else class="bt-peer-table__none">—</span>
            </td>
            <td class="bt-peer-table__cell bt-peer-table__cell--flags">
              <span v-if="peer.flags" class="bt-peer-table__flags">
                <UiBadge
                  v-for="(flag, fi) in peer.flags.split('')"
                  :key="fi"
                  size="sm"
                  tone="neutral"
                >
                  {{ flag }}
                </UiBadge>
              </span>
              <span v-else class="bt-peer-table__none">—</span>
            </td>
            <td class="bt-peer-table__cell bt-peer-table__cell--dl-speed">
              {{ formatPeerSpeed(peer.downloadSpeed) }}
            </td>
            <td class="bt-peer-table__cell bt-peer-table__cell--ul-speed">
              {{ formatPeerSpeed(peer.uploadSpeed) }}
            </td>
            <td class="bt-peer-table__cell bt-peer-table__cell--progress">
              <div class="bt-peer-table__progress">
                <UiProgress :value="peerProgress(peer)" />
                <span class="bt-peer-table__progress-label">{{ peerProgress(peer) }}%</span>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.bt-peer-table {
  display: grid;
  gap: var(--space-2);
}

/* ── Empty state ── */

.bt-peer-table__empty {
  display: grid;
  place-items: center;
  min-height: 8rem;
  color: var(--color-text-muted);
  border: 1px dashed var(--color-border-strong);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
}

.bt-peer-table__empty p {
  margin: 0;
  font-size: var(--font-size-small);
}

/* ── Table shell ── */

.bt-peer-table__shell {
  border-top: 1px solid var(--color-border);
  border-bottom: 1px solid var(--color-border);
  overflow: hidden;
  background: transparent;
}

/* ── Table ── */

.bt-peer-table__table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

/* ── Header ── */

.bt-peer-table__th {
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

/* ── Column widths ── */

.bt-peer-table__col--ip {
  width: 22%;
}

.bt-peer-table__col--client {
  width: 22%;
}

.bt-peer-table__col--flags {
  width: 16%;
}

.bt-peer-table__col--dl-speed {
  width: 14%;
}

.bt-peer-table__col--ul-speed {
  width: 14%;
}

.bt-peer-table__col--progress {
  width: 12%;
}

/* ── Rows ── */

.bt-peer-table__row + .bt-peer-table__row .bt-peer-table__cell {
  border-top: 1px solid var(--color-border);
}

.bt-peer-table__cell {
  padding: 0.3rem 0.75rem;
  vertical-align: middle;
}

.bt-peer-table__cell--address,
.bt-peer-table__cell--dl-speed,
.bt-peer-table__cell--ul-speed {
  color: var(--color-text-main);
  font-size: 0.82rem;
}

.bt-peer-table__cell--address {
  font-family: var(--font-mono, "Cascadia Code", "Fira Code", monospace);
  font-size: 0.78rem;
}

.bt-peer-table__cell--client {
  font-size: 0.8rem;
}

.bt-peer-table__cell--dl-speed,
.bt-peer-table__cell--ul-speed {
  font-size: 0.8125rem;
  color: var(--color-text-muted);
}

/* ── Client name ── */
.bt-peer-table__client-name {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── Placeholder ── */

.bt-peer-table__none {
  color: var(--color-text-soft);
}

/* ── Flags ── */

.bt-peer-table__flags {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  flex-wrap: wrap;
}

/* ── Progress ── */

.bt-peer-table__progress {
  display: grid;
  gap: 0.25rem;
}

.bt-peer-table__progress-label {
  color: var(--color-text-muted);
  font-size: 0.72rem;
}
</style>
