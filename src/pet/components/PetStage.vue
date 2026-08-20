<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from "vue";
import {
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
  PhysicalSize,
} from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import PetSprite from "./PetSprite.vue";
import PetContextMenu from "./PetContextMenu.vue";
import { usePetBehavior } from "../composables/usePetBehavior";
import { usePetFps } from "../composables/usePetFps";
import {
  petSetIgnoreCursorEvents,
  petUpdatePosition,
  petGetMenuState,
  petTogglePauseAll,
  petToggleSpeedLimit,
  petToggleGameMode,
  petOpenDownloadDir,
  petShowMain,
  petOpenSettings,
  petSetEnabled,
  petQuit,
  type PetMenuState,
} from "../../lib/tauri/pet-api";
import { startDownload } from "../../lib/tauri/download-api";
import type { PetSettings } from "../../types/settings";
import type { StartDownloadRequest } from "../../types/generated/types";

const props = defineProps<{
  settings: PetSettings;
}>();

const { state, onDragStart, onDragEnd, onDropSuccess } = usePetBehavior();
const { frame } = usePetFps(state);

const baseSize = computed(() => 160 * (props.settings.scale ?? 1));

const stageRef = ref<HTMLDivElement | null>(null);
void stageRef;
const isHovering = ref(false);
const isDragging = ref(false);
const dragOver = ref(false);
const dropMessage = ref<string | null>(null);
let dropMsgTimer: number | null = null;
let positionSaveTimer: number | null = null;

// Context menu state
const showMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const menuState = ref<PetMenuState | null>(null);
let originalSize: { w: number; h: number } | null = null;

// Hover → toggle cursor events
// 临时修复：Windows 下 ignore=true 会导致收不到 mouseenter 从而无法拖动，
// 骨架阶段让桌宠始终可交互（会挡住 160px 区域的桌面点击，但保证可拖）
async function setHover(hover: boolean) {
  isHovering.value = hover;
  try {
    await petSetIgnoreCursorEvents(false);
  } catch {
    // ignore on NAS or when window not available
  }
}

async function handleMouseDown(e: MouseEvent) {
  if (showMenu.value) {
    closeMenu();
    return;
  }
  if (e.button !== 0) return;
  isDragging.value = true;
  onDragStart();
  void petSetIgnoreCursorEvents(false);

  // Manual JS drag — far more reliable than OS start_dragging which swallows mouseup
  // and left the previous implementation stuck in drag forever.
  const win = getCurrentWindow();
  let startPos: { x: number; y: number } | null = null;
  let startMouse: { x: number; y: number } | null = null;
  let scale = 1;
  try {
    const pos = await win.outerPosition();
    startPos = { x: pos.x, y: pos.y };
    scale = await win.scaleFactor();
    startMouse = { x: e.screenX * scale, y: e.screenY * scale };
  } catch {
    // fallback: use client position only
    startPos = null;
  }

  let pendingPos: { x: number; y: number } | null = null;
  let rafId: number | null = null;

  const flushPos = () => {
    if (pendingPos) {
      void win.setPosition(new PhysicalPosition(pendingPos.x, pendingPos.y));
      pendingPos = null;
    }
    rafId = null;
  };

  const onMouseMove = (ev: MouseEvent) => {
    if (!isDragging.value || !startPos || !startMouse) return;
    const curX = ev.screenX * scale;
    const curY = ev.screenY * scale;
    const dx = curX - startMouse.x;
    const dy = curY - startMouse.y;
    pendingPos = { x: Math.round(startPos.x + dx), y: Math.round(startPos.y + dy) };
    if (rafId === null) {
      rafId = window.requestAnimationFrame(flushPos);
    }
  };

  const cleanup = () => {
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    if (rafId !== null) {
      window.cancelAnimationFrame(rafId);
      rafId = null;
    }
    pendingPos = null;
  };

  const onMouseUp = () => {
    cleanup();
    if (!isDragging.value) return;
    // Flush any pending position before ending drag
    if (pendingPos) {
      void win.setPosition(new PhysicalPosition(pendingPos.x, pendingPos.y));
      pendingPos = null;
    }
    isDragging.value = false;
    onDragEnd();
    void savePosition();
  };

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp, { once: true });

  // Safety: if for any reason mouseup is missed, also end drag on window blur / escape
  const onKey = (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      cleanup();
      if (isDragging.value) {
        isDragging.value = false;
        onDragEnd();
      }
    }
  };
  window.addEventListener("keydown", onKey, { once: true });
}

async function expandForMenu() {
  try {
    const win = getCurrentWindow();
    const size = await win.innerSize();
    originalSize = { w: size.width, h: size.height };
    // Expand to fit menu below the pet — use logical size so it scales with DPI
    // Pet is baseSize (160*scale) at top, menu 200x~300 below needs baseSize+320
    const targetW = 260;
    const targetH = Math.ceil(baseSize.value + 320);
    const factor = await win.scaleFactor();
    const curW = size.width / factor;
    const curH = size.height / factor;
    if (curW < targetW || curH < targetH) {
      await win.setSize(new LogicalSize(targetW, targetH));
    }
  } catch {
    // ignore
  }
}

async function restoreFromMenu() {
  if (originalSize) {
    try {
      const win = getCurrentWindow();
      await win.setSize(new PhysicalSize(originalSize.w, originalSize.h));
    } catch {
      // ignore
    }
    originalSize = null;
  }
}

async function handleContextMenu(e: MouseEvent) {
  e.preventDefault();
  // Fetch menu state for checkmarks
  try {
    menuState.value = await petGetMenuState();
  } catch {
    menuState.value = null;
  }
  // Expand first so the window can host the menu without clipping
  await expandForMenu();
  // Fixed position below the pet — keeps the cat from shifting
  // and guarantees the menu is fully visible in the expanded window
  menuX.value = 20;
  menuY.value = Math.ceil(baseSize.value + 8);
  showMenu.value = true;
  void petSetIgnoreCursorEvents(false);
}

function closeMenu() {
  showMenu.value = false;
  void restoreFromMenu();
  // 保持可交互
  void petSetIgnoreCursorEvents(false);
}

async function handleMenuAction(id: string) {
  try {
    switch (id) {
      case "show_main":
        await petShowMain();
        break;
      case "pause_all":
        await petTogglePauseAll();
        break;
      case "speed_limit":
        await petToggleSpeedLimit();
        break;
      case "game_mode":
        await petToggleGameMode();
        break;
      case "open_dir":
        await petOpenDownloadDir();
        break;
      case "settings":
        await petOpenSettings();
        break;
      case "hide_pet":
        await petSetEnabled(false);
        break;
      case "quit":
        await petQuit();
        break;
    }
  } catch (err) {
    console.error("[pet] menu action failed", id, err);
  }
  closeMenu();
}

async function savePosition() {
  try {
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    // Debounce save
    if (positionSaveTimer !== null) window.clearTimeout(positionSaveTimer);
    positionSaveTimer = window.setTimeout(() => {
      void petUpdatePosition(pos.x, pos.y);
    }, 500);
  } catch {
    // ignore
  }
}

// Drag & drop handling — drop links/files onto pet to create download
function isValidUrl(text: string): boolean {
  const t = text.trim();
  return (
    t.startsWith("http://") ||
    t.startsWith("https://") ||
    t.startsWith("magnet:?") ||
    t.toLowerCase().endsWith(".torrent")
  );
}

function showDropMessage(msg: string) {
  dropMessage.value = msg;
  if (dropMsgTimer !== null) window.clearTimeout(dropMsgTimer);
  dropMsgTimer = window.setTimeout(() => {
    dropMessage.value = null;
  }, 2500);
}

async function handleDragOver(e: DragEvent) {
  e.preventDefault();
  dragOver.value = true;
}

function handleDragLeave() {
  dragOver.value = false;
}

async function handleDrop(e: DragEvent) {
  e.preventDefault();
  dragOver.value = false;

  const text =
    e.dataTransfer?.getData("text/uri-list") ||
    e.dataTransfer?.getData("text/plain") ||
    e.dataTransfer?.getData("text") ||
    "";

  // Also check files (e.g., .torrent file)
  const files = e.dataTransfer?.files;
  let url = text.trim();

  if (!url && files && files.length > 0) {
    // For file drop, the path is not directly available in webview.
    // We rely on Rust's DragDropEvent for file paths; here we just show hint.
    showDropMessage("文件拖拽请通过系统拖拽（占位）");
    return;
  }

  if (!url) {
    showDropMessage("未识别到链接");
    return;
  }

  // Extract first valid URL if multiple lines
  const lines = url
    .split(/[\r\n]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  const valid = lines.find((l) => isValidUrl(l)) || lines[0];
  if (!isValidUrl(valid)) {
    showDropMessage("不支持的链接类型");
    return;
  }

  try {
    const req: StartDownloadRequest = {
      url: valid,
      destinationDir: "",
      kind: null,
      fileName: null,
      userAgent: null,
      threadMode: null,
      threadCount: null,
      maxRetries: null,
      checksum: null,
      expectedChecksum: null,
      selectedFileIndices: null,
      headers: null,
      startPaused: false,
      mirrorUrls: null,
      priority: null,
    };
    await startDownload(req);
    onDropSuccess();
    showDropMessage("已添加下载 ✨");
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    showDropMessage(`添加失败: ${msg.slice(0, 40)}`);
  }
}

onMounted(async () => {
  // 始终可交互，保证拖动可用
  void setHover(true);

  // Listen to Tauri drag-drop events for file paths (fallback)
  try {
    const unlisten = await listen("tauri://drag-enter", () => {
      dragOver.value = true;
      void setHover(true);
    });
    const un2 = await listen("tauri://drag-leave", () => {
      dragOver.value = false;
    });
    const un3 = await listen("tauri://drag-drop", () => {
      dragOver.value = false;
    });
    // Store for cleanup
    onUnmounted(() => {
      unlisten();
      un2();
      un3();
    });
  } catch {
    // ignore on non-tauri
  }
});

onUnmounted(() => {
  if (dropMsgTimer !== null) window.clearTimeout(dropMsgTimer);
  if (positionSaveTimer !== null) window.clearTimeout(positionSaveTimer);
  showMenu.value = false;
});
</script>

<template>
  <div
    ref="stageRef"
    class="pet-stage"
    :style="{ width: `${baseSize}px`, height: `${baseSize}px` }"
    :class="{
      'is-drag-over': dragOver,
      'is-dragging': isDragging,
      'is-transparent': settings.transparentBackground,
    }"
    @mouseenter="setHover(true)"
    @mouseleave="setHover(false)"
    @mousedown="handleMouseDown"
    @contextmenu="handleContextMenu"
    @dragover="handleDragOver"
    @dragleave="handleDragLeave"
    @drop="handleDrop"
  >
    <PetSprite :state="state" :frame="frame" :transparent="settings.transparentBackground" />

    <Transition name="fade">
      <div v-if="dropMessage" class="pet-bubble">
        {{ dropMessage }}
      </div>
    </Transition>

    <div v-if="dragOver" class="pet-drop-hint">松开添加下载</div>

    <PetContextMenu
      :x="menuX"
      :y="menuY"
      :state="menuState"
      :visible="showMenu"
      @close="closeMenu"
      @action="handleMenuAction"
    />
  </div>
</template>

<style scoped>
.pet-stage {
  position: absolute;
  left: 0;
  top: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 16px;
  transition: background 0.2s;
}

.pet-stage.is-drag-over {
  background: rgba(132, 204, 2, 0.12);
  outline: 2px dashed rgba(132, 204, 2, 0.6);
  outline-offset: -4px;
}

.pet-stage.is-dragging {
  opacity: 0.9;
}

.pet-bubble {
  position: absolute;
  top: 8px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.78);
  color: #fff;
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 10px;
  white-space: nowrap;
  pointer-events: none;
}

.pet-drop-hint {
  position: absolute;
  bottom: 6px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 11px;
  color: #5a7a00;
  background: rgba(255, 255, 255, 0.9);
  padding: 2px 8px;
  border-radius: 8px;
  pointer-events: none;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
