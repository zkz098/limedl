<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import PetSprite from "./PetSprite.vue";
import { usePetBehavior } from "../composables/usePetBehavior";
import { usePetFps } from "../composables/usePetFps";
import { petStartDrag, petSetIgnoreCursorEvents, petUpdatePosition } from "../../lib/tauri/pet-api";
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

// Hover → toggle cursor events (穿透逻辑)
async function setHover(hover: boolean) {
  isHovering.value = hover;
  try {
    // hovering = 可交互 (ignore=false), not hovering = 穿透 (ignore=true)
    await petSetIgnoreCursorEvents(!hover && !isDragging.value);
  } catch {
    // ignore on NAS or when window not available
  }
}

async function handleMouseDown(e: MouseEvent) {
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

function handleContextMenu(e: MouseEvent) {
  e.preventDefault();
  // TODO: show native menu via Rust (popup_menu). Skeleton keeps default.
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
  // Initial state:穿透，悬停才交互
  void setHover(false);

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
