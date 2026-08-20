<script setup lang="ts">
import { computed } from "vue";
import type { PetState } from "../composables/usePetBehavior";

const props = defineProps<{
  state: PetState;
  frame: number;
}>();

// Placeholder sprite: we use CSS + emoji transform instead of real sheet.
// Replace `background-position` with sprite sheet when assets are ready.
const label = computed(() => {
  switch (props.state) {
    case "work":
      return "🔨";
    case "celebrate":
      return "🎉";
    case "sad":
      return "😿";
    case "drag":
      return "✋";
    case "sleep":
      return "😴";
    case "idle":
    default:
      return "🐱";
  }
});

const animationClass = computed(() => {
  switch (props.state) {
    case "work":
      return "anim-bounce";
    case "celebrate":
      return "anim-pop";
    case "sleep":
      return "anim-breathe";
    case "drag":
      return "anim-drag";
    default:
      return "anim-idle";
  }
});

// Subtle frame-driven offset to show FPS is working (placeholder)
const offset = computed(() => {
  if (props.state === "work" || props.state === "celebrate") {
    return props.frame % 2 === 0 ? 0 : 1;
  }
  return 0;
});
</script>

<template>
  <div class="pet-sprite" :class="animationClass" :style="{ transform: `translateY(${offset}px)` }">
    <div class="pet-emoji">{{ label }}</div>
    <div class="pet-shadow" />
    <!-- TODO: replace with sprite sheet
    <div
      class="pet-sheet"
      :style="{
        backgroundImage: `url(${sheetUrl})`,
        backgroundPosition: `${-frame * 160}px 0`,
      }"
    />
    -->
    <div class="pet-state-label">{{ state }}</div>
  </div>
</template>

<style scoped>
.pet-sprite {
  width: 140px;
  height: 140px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border-radius: 24px;
  background: rgba(255, 255, 255, 0.92);
  border: 1px solid rgba(0, 0, 0, 0.06);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.12),
    0 2px 8px rgba(0, 0, 0, 0.08);
  position: relative;
  cursor: grab;
  user-select: none;
}

.pet-sprite:active {
  cursor: grabbing;
}

.pet-emoji {
  font-size: 56px;
  line-height: 1;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.1));
}

.pet-shadow {
  margin-top: 8px;
  width: 60px;
  height: 8px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.08);
}

.pet-state-label {
  position: absolute;
  bottom: 6px;
  font-size: 10px;
  color: #888;
  background: rgba(255, 255, 255, 0.8);
  padding: 2px 6px;
  border-radius: 8px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

/* Placeholder animations — replace with sprite steps() later */
.anim-idle {
  animation: pet-idle 2s ease-in-out infinite;
}

.anim-bounce {
  animation: pet-bounce 0.4s ease-in-out infinite;
}

.anim-pop {
  animation: pet-pop 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) infinite;
}

.anim-breathe {
  animation: pet-breathe 3s ease-in-out infinite;
}

.anim-drag {
  opacity: 0.9;
  transform: scale(1.05) !important;
}

@keyframes pet-idle {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-4px);
  }
}

@keyframes pet-bounce {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-6px) scale(1.02);
  }
}

@keyframes pet-pop {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.08);
  }
  100% {
    transform: scale(1);
  }
}

@keyframes pet-breathe {
  0%,
  100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.03);
  }
}
</style>
