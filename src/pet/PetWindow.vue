<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { listen } from "#event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import PetStage from "./components/PetStage.vue";
import { getAppSettings } from "../lib/tauri/settings-api";
import type { AppSettings, PetSettings } from "../types/settings";

const petSettings = ref<PetSettings | null>(null);
const isReady = ref(false);

async function loadPetSettings() {
  try {
    const settings = await getAppSettings();
    petSettings.value = settings.pet;
  } catch (e) {
    console.error("[pet] failed to load settings", e);
    petSettings.value = {
      enabled: true,
      scale: 1,
      opacity: 1,
      keepAliveWhenMainHidden: true,
      model: "default",
      transparentBackground: false,
    } as PetSettings;
  } finally {
    isReady.value = true;
  }
}

// React to scale changes without restart — resize window immediately
watch(
  () => petSettings.value?.scale,
  async (scale) => {
    if (!scale || !petSettings.value) return;
    try {
      const win = getCurrentWindow();
      const size = 160 * scale;
      await win.setSize(new LogicalSize(size, size));
    } catch {
      // ignore
    }
  },
);

let unlistenSettings: (() => void) | null = null;
let unlistenPet: (() => void) | null = null;

onMounted(async () => {
  await loadPetSettings();
  // Live-update when main window saves settings (no restart needed)
  try {
    unlistenSettings = await listen<AppSettings>("settings-updated", (event) => {
      const pet = (event.payload as AppSettings).pet;
      if (pet) petSettings.value = pet;
    });
    unlistenPet = await listen<PetSettings>("pet-settings-updated", (event) => {
      petSettings.value = event.payload as PetSettings;
    });
  } catch {
    // ignore on NAS / test
  }
});

onUnmounted(() => {
  unlistenSettings?.();
  unlistenPet?.();
});
</script>

<template>
  <div class="pet-root" :style="{ opacity: petSettings?.opacity ?? 1 }">
    <PetStage v-if="isReady" :settings="petSettings!" />
    <div v-else class="pet-loading">loading...</div>
  </div>
</template>

<style>
html,
body {
  background: transparent !important;
  overflow: hidden;
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
}

#pet-app {
  width: 100vw;
  height: 100vh;
  background: transparent;
}
</style>

<style scoped>
.pet-root {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  user-select: none;
}

.pet-loading {
  font-size: 12px;
  color: #888;
}
</style>
