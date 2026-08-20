<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import PetStage from "./components/PetStage.vue";
import { getAppSettings } from "../lib/tauri/settings-api";
import type { PetSettings } from "../types/settings";

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
    } as PetSettings;
  } finally {
    isReady.value = true;
  }
}

onMounted(() => {
  void loadPetSettings();
});

onUnmounted(() => {});
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
