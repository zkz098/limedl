<script setup lang="ts">
import type { PetMenuState } from "../../lib/tauri/pet-api";

defineProps<{
  x: number;
  y: number;
  state: PetMenuState | null;
  visible: boolean;
}>();

const emit = defineEmits<{
  close: [];
  action: [id: string];
}>();

function onAction(id: string) {
  emit("action", id);
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <Transition name="pet-menu-fade">
      <div
        v-if="visible"
        class="pet-menu-overlay"
        @click="emit('close')"
        @contextmenu.prevent="emit('close')"
      >
        <div
          class="pet-menu"
          :style="{ left: `${x}px`, top: `${y}px` }"
          role="menu"
          @click.stop
          @contextmenu.prevent
        >
          <!-- Section: Main window -->
          <button role="menuitem" class="pet-menu__item" @click="onAction('show_main')">
            <span class="i-ri-window-line pet-menu__icon" aria-hidden="true" />
            <span>显示主窗口</span>
          </button>

          <div class="pet-menu__sep" role="separator" />

          <!-- Section: Batch controls -->
          <button role="menuitem" class="pet-menu__item" @click="onAction('pause_all')">
            <span
              :class="state?.hasActive ? 'i-ri-pause-line' : 'i-ri-play-line'"
              class="pet-menu__icon"
              aria-hidden="true"
            />
            <span>{{ state?.hasActive ? "暂停全部" : "恢复全部" }}</span>
          </button>

          <button
            role="menuitemcheckbox"
            class="pet-menu__item"
            :aria-checked="state?.speedLimitActive ?? false"
            @click="onAction('speed_limit')"
          >
            <span class="i-ri-speed-up-line pet-menu__icon" aria-hidden="true" />
            <span>限速模式</span>
            <span
              v-if="state?.speedLimitActive"
              class="i-ri-check-line pet-menu__check"
              aria-hidden="true"
            />
          </button>

          <button
            role="menuitemcheckbox"
            class="pet-menu__item"
            :aria-checked="state?.gameMode ?? false"
            @click="onAction('game_mode')"
          >
            <span class="i-ri-gamepad-line pet-menu__icon" aria-hidden="true" />
            <span>游戏模式</span>
            <span
              v-if="state?.gameMode"
              class="i-ri-check-line pet-menu__check"
              aria-hidden="true"
            />
          </button>

          <div class="pet-menu__sep" role="separator" />

          <button role="menuitem" class="pet-menu__item" @click="onAction('open_dir')">
            <span class="i-ri-folder-open-line pet-menu__icon" aria-hidden="true" />
            <span>打开下载目录</span>
          </button>

          <button role="menuitem" class="pet-menu__item" @click="onAction('settings')">
            <span class="i-ri-settings-3-line pet-menu__icon" aria-hidden="true" />
            <span>桌宠设置…</span>
          </button>

          <div class="pet-menu__sep" role="separator" />

          <button
            role="menuitem"
            class="pet-menu__item pet-menu__item--muted"
            @click="onAction('hide_pet')"
          >
            <span class="i-ri-eye-off-line pet-menu__icon" aria-hidden="true" />
            <span>隐藏桌宠</span>
          </button>

          <button
            role="menuitem"
            class="pet-menu__item pet-menu__item--danger"
            @click="onAction('quit')"
          >
            <span class="i-ri-logout-box-line pet-menu__icon" aria-hidden="true" />
            <span>退出应用</span>
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.pet-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
}

.pet-menu {
  position: fixed;
  min-width: 192px;
  max-width: 240px;
  padding: 6px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  backdrop-filter: blur(12px) saturate(1.2);
  border: 1px solid rgba(0, 0, 0, 0.08);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.14),
    0 2px 8px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.pet-menu__item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 30px;
  padding: 6px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #1a1a1a;
  font-size: 13px;
  line-height: 1;
  text-align: left;
  cursor: pointer;
  user-select: none;
}

.pet-menu__item:hover,
.pet-menu__item:focus-visible {
  background: rgba(132, 204, 2, 0.12);
  color: #1a1a1a;
  outline: none;
}

.pet-menu__item:active {
  background: rgba(132, 204, 2, 0.18);
}

.pet-menu__item--muted {
  color: #666;
}

.pet-menu__item--danger {
  color: #c0392b;
}

.pet-menu__item--danger:hover,
.pet-menu__item--danger:focus-visible {
  background: rgba(192, 57, 43, 0.08);
  color: #a93226;
}

.pet-menu__icon {
  flex: 0 0 auto;
  font-size: 15px;
  opacity: 0.9;
}

.pet-menu__check {
  margin-left: auto;
  font-size: 14px;
  color: #6a9a00;
}

.pet-menu__sep {
  height: 1px;
  margin: 4px 6px;
  background: rgba(0, 0, 0, 0.06);
}

.pet-menu-fade-enter-active,
.pet-menu-fade-leave-active {
  transition:
    opacity 0.14s ease,
    transform 0.14s ease;
}

.pet-menu-fade-enter-from,
.pet-menu-fade-leave-to {
  opacity: 0;
  transform: scale(0.98);
}
</style>
