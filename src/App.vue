<script setup lang="ts">
import { ref, computed } from "vue";
import Button from "primevue/button";
import Dialog from "primevue/dialog";

import DownloadComposer from "./components/downloader/DownloadComposer.vue";
import DownloadInspector from "./components/downloader/DownloadInspector.vue";
import DownloadQueueTable from "./components/downloader/DownloadQueueTable.vue";
import { formatSpeed, stateLabel } from "./lib/download-format";
import { useDownloader } from "./composables/useDownloader";
import type { ChecksumMode } from "./types/download";

const checksumOptions: { label: string; value: ChecksumMode }[] = [
  { label: "BLAKE3", value: "blake3" },
  { label: "None", value: "none" },
];

const {
  actionName,
  canCancel,
  canPause,
  canResume,
  downloads,
  errorMessage,
  form,
  infoMessage,
  isAutoRefreshing,
  isPickingDirectory,
  isRefreshingList,
  isRefreshingStatus,
  isStarting,
  pickDestinationDirectory,
  refreshList,
  refreshStatus,
  runCancel,
  runPause,
  runResume,
  selectDownload,
  selectedId,
  selectedSnapshot,
  selectedSummary,
  submitStart,
} = useDownloader();

const showComposerDialog = ref(false);

const selectedOverview = computed(() => selectedSnapshot.value ?? selectedSummary.value);
const selectedStateLabel = computed(() => stateLabel(selectedOverview.value?.state));
const activeSpeedLabel = computed(() => formatSpeed(selectedOverview.value?.speedBytesPerSecond));

const handleSubmitStart = async () => {
  await submitStart();
  showComposerDialog.value = false;
};
</script>

<template>
  <main class="app-shell">
    <aside class="sidebar">
      <div class="sidebar-brand">
        <h1>Downloader</h1>
      </div>
      <div class="sidebar-actions">
        <Button class="new-task-btn" icon="pi pi-plus" label="新建任务" @click="showComposerDialog = true" />
      </div>
      <nav class="sidebar-nav">
        <div class="nav-item active">
          任务列表
        </div>
      </nav>
      <div class="sidebar-stats">
        <div class="stat-item">
          <span>总任务</span>
          <strong>{{ downloads.length }}</strong>
        </div>
        <div class="stat-item">
          <span>当前速度</span>
          <strong>{{ activeSpeedLabel }}</strong>
        </div>
        <div class="stat-item">
          <span>选中状态</span>
          <strong>{{ selectedStateLabel }}</strong>
        </div>
      </div>
    </aside>

    <section class="main-content">
      <DownloadQueueTable
        :downloads="downloads"
        :error-message="errorMessage"
        :info-message="infoMessage"
        :is-auto-refreshing="isAutoRefreshing"
        :is-refreshing-list="isRefreshingList"
        :selected-id="selectedId"
        @refresh="refreshList"
        @select="selectDownload"
      />
      <transition name="slide-up">
        <DownloadInspector
          v-if="selectedId"
          class="floating-inspector"
          :action-name="actionName"
          :can-cancel="canCancel"
          :can-pause="canPause"
          :can-resume="canResume"
          :is-refreshing-status="isRefreshingStatus"
          :selected-overview="selectedOverview"
          :selected-snapshot="selectedSnapshot"
          @cancel="runCancel"
          @pause="runPause"
          @refresh="refreshStatus"
          @resume="runResume"
          @close="selectDownload(null)"
        />
      </transition>
    </section>

    <Dialog v-model:visible="showComposerDialog" modal header="新建任务" :style="{ width: '50vw' }" :breakpoints="{ '1199px': '75vw', '575px': '90vw' }">
      <DownloadComposer
        :checksum-options="checksumOptions"
        :form="form"
        :is-picking-directory="isPickingDirectory"
        :is-starting="isStarting"
        @pick-directory="pickDestinationDirectory"
        @submit="handleSubmitStart"
      />
    </Dialog>
  </main>
</template>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  width: 100%;
}

.sidebar {
  width: var(--sidebar-width);
  background: var(--color-bg-sidebar);
  border-right: var(--border-width-thin) solid var(--color-border);
  display: flex;
  flex-direction: column;
  padding: var(--space-4);
  gap: var(--space-5);
}

.sidebar-brand h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--font-size-hero);
  color: var(--color-accent-strong);
  line-height: var(--line-height-display);
}

.new-task-btn {
  width: 100%;
  border-radius: var(--radius-round) !important;
  font-weight: bold;
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1;
}

.nav-item {
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
  transition: background var(--duration-fast);
}

.nav-item:hover {
  background: var(--color-surface-hover);
}

.nav-item.active {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
}

.sidebar-stats {
  display: grid;
  gap: var(--space-3);
  padding-top: var(--space-4);
  border-top: var(--border-width-thin) solid var(--color-border);
}

.stat-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.stat-item strong {
  color: var(--color-text-main);
}

.main-content {
  flex: 1;
  min-width: 0;
  padding: var(--space-5);
  background: var(--color-bg-base);
  overflow-y: auto;
  position: relative;
  display: flex;
  flex-direction: column;
}

.floating-inspector {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 10;
  background: var(--color-bg-panel);
  border-top: var(--border-width-thin) solid var(--color-border);
  box-shadow: 0 -4px 16px rgba(0, 0, 0, 0.08); /* light floating shadow */
  max-height: 50vh;
  overflow-y: auto;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
}
</style>
