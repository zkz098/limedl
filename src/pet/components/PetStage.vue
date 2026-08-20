<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { getCurrentWindow, LogicalSize, PhysicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import PetSprite from "./PetSprite.vue";
import PetContextMenu from "./PetContextMenu.vue";
import { usePetBehavior } from "../composables/usePetBehavior";
import { usePetFps } from "../composables/usePetFps";
import {
  petStartDrag,
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
  try {
    await petSetIgnoreCursorEvents(false);
    await petStartDrag();
  } catch (err) {
    console.error("[pet] start_drag failed", err);
  }
  // Tauri's start_dragging blocks until drop; we poll position afterwards
  // Use a small delay to detect drag end
  window.setTimeout(() => {
    isDragging.value = false;
    onDragEnd();
    void savePosition();
    void setHover(false);
  }, 300);
}

async function expandForMenu() {
  try {
    const win = getCurrentWindow();
    const size = await win.innerSize();
    originalSize = { w: size.width, h: size.height };
    // Expand to fit menu — use logical size so it scales with DPI
    const target = new LogicalSize(240, 380);
    const factor = await win.scaleFactor();
    const curW = size.width / factor;
    const curH = size.height / factor;
    if (curW < 240 || curH < 380) {
      await win.setSize(target);
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
  // Position inside window (clamped)
  const winW = window.innerWidth;
  const winH = window.innerHeight;
  // Menu visual size ~ 192x320, but window will be expanded
  let x = e.clientX;
  let y = e.clientY;
  // After expand, window is 240x380, so clamp to that expanded size
  const menuW = 200;
  const menuH = 300;
  const expandedW = 240;
  const expandedH = 380;
  // Clamp to expanded window bounds with 6px margin
  x = Math.min(Math.max(6, x), expandedW - menuW - 6);
  y = Math.min(Math.max(6, y), expandedH - menuH - 6);
  // If window not yet expanded, use current window size for clamping fallback
  if (winW < expandedW) {
    x = Math.min(x, winW - menuW - 6);
    y = Math.min(y, winH - menuH - 6);
  }
  menuX.value = x;
  menuY.value = y;
  showMenu.value = true;
  void expandForMenu();
  // Ensure pet is interactive while menu open
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
    :class="{ 'is-drag-over': dragOver, 'is-dragging': isDragging }"
    @mouseenter="setHover(true)"
    @mouseleave="setHover(false)"
    @mousedown="handleMouseDown"
    @contextmenu="handleContextMenu"
    @dragover="handleDragOver"
    @dragleave="handleDragLeave"
    @drop="handleDrop"
  >
    <PetSprite :state="state" :frame="frame" />

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
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
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
