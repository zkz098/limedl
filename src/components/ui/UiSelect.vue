<script lang="ts">
let listboxIdCounter = 0;
</script>

<script setup lang="ts" generic="T extends string | number | null">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { onClickOutside } from "@vueuse/core";

const props = withDefaults(
  defineProps<{
    modelValue: T;
    options: { label: string; value: T }[];
    id?: string;
    ariaLabel?: string;
    ariaLabelledby?: string;
    disabled?: boolean;
    placeholder?: string;
  }>(),
  {
    disabled: false,
    placeholder: "Select…",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: T];
}>();

const listboxId = `ui-select-listbox-${listboxIdCounter++}`;

const isOpen = ref(false);
const triggerRef = ref<HTMLButtonElement | null>(null);
const panelRef = ref<HTMLDivElement | null>(null);
const optionRefs = ref<HTMLDivElement[]>([]);
const activeIndex = ref(0);
const panelStyle = ref({ top: "0px", left: "0px", width: "0px" });

const typeAhead = ref("");
let typeAheadTimer: ReturnType<typeof setTimeout> | undefined;

const selectedIndex = computed(() =>
  props.options.findIndex((option) => option.value === props.modelValue),
);

const selectedOption = computed(() =>
  selectedIndex.value >= 0 ? props.options[selectedIndex.value] : undefined,
);

const displayLabel = computed(() => selectedOption.value?.label ?? props.placeholder);

const activeDescendantId = computed(() =>
  isOpen.value ? `${listboxId}-option-${activeIndex.value}` : undefined,
);

function setOptionRef(el: unknown, index: number) {
  if (el instanceof HTMLElement) {
    optionRefs.value[index] = el as HTMLDivElement;
  }
}

function pickVerticalTop(
  triggerBottom: number,
  triggerTop: number,
  panelHeight: number,
  gap: number,
  viewportH: number,
): number {
  let top = triggerBottom + gap;
  const spaceBelow = viewportH - triggerBottom - gap;
  const spaceAbove = triggerTop - gap;

  // Flip above if panel overflows bottom and there's more room above
  if (panelHeight > spaceBelow && spaceAbove >= spaceBelow) {
    top = triggerTop - panelHeight - gap;
  }

  // Clamp vertical position within viewport
  if (top < gap) top = gap;
  if (top + panelHeight > viewportH) {
    top = viewportH - panelHeight - gap;
  }
  return top;
}

function clampHorizontal(left: number, width: number, viewportW: number, gap: number): number {
  let result = left;
  if (result + width > viewportW) {
    result = viewportW - width - gap;
  }
  if (result < gap) result = gap;
  return result;
}

function updatePosition() {
  if (!isOpen.value || !triggerRef.value) return;

  const triggerRect = triggerRef.value.getBoundingClientRect();
  const panelEl = panelRef.value;
  const gap = 4;
  const viewportW = window.innerWidth;
  const viewportH = window.innerHeight;
  const width = triggerRect.width;

  let top = triggerRect.bottom + gap;
  let left = triggerRect.left;

  if (panelEl) {
    const panelHeight = panelEl.clientHeight || panelEl.scrollHeight;
    if (panelHeight > 0) {
      top = pickVerticalTop(triggerRect.bottom, triggerRect.top, panelHeight, gap, viewportH);
      left = clampHorizontal(left, width, viewportW, gap);
    }
  }

  panelStyle.value = {
    top: `${top}px`,
    left: `${left}px`,
    width: `${width}px`,
  };
}

function focusOption(index: number) {
  if (index < 0 || index >= props.options.length) return;

  nextTick(() => {
    const optionEl = optionRefs.value[index];
    optionEl?.focus();
    optionEl?.scrollIntoView({ block: "nearest" });
  });
}

function navigate(dir: number) {
  if (props.options.length === 0) return;

  let next = activeIndex.value + dir;
  if (next < 0) next = props.options.length - 1;
  if (next >= props.options.length) next = 0;

  activeIndex.value = next;
  focusOption(next);
}

function selectOption(index: number) {
  if (index < 0 || index >= props.options.length) return;

  emit("update:modelValue", props.options[index].value);
}

function handleType(char: string) {
  if (props.options.length === 0) return;

  typeAhead.value += char;
  const query = typeAhead.value.toLowerCase();
  const match = props.options.findIndex((option) => option.label.toLowerCase().startsWith(query));

  if (match !== -1) {
    activeIndex.value = match;
    focusOption(match);
  }

  if (typeAheadTimer) clearTimeout(typeAheadTimer);
  typeAheadTimer = window.setTimeout(() => {
    typeAhead.value = "";
  }, 500);
}

function open() {
  if (props.disabled || props.options.length === 0) return;

  isOpen.value = true;
  activeIndex.value = selectedIndex.value >= 0 ? selectedIndex.value : 0;

  nextTick(() => {
    updatePosition();
    focusOption(activeIndex.value);
  });
}

function close() {
  if (!isOpen.value) return;

  isOpen.value = false;
  if (typeAheadTimer) clearTimeout(typeAheadTimer);
  typeAhead.value = "";

  nextTick(() => {
    triggerRef.value?.focus();
  });
}

function toggle() {
  if (props.disabled) return;
  if (isOpen.value) {
    close();
  } else {
    open();
  }
}

function openIfClosed() {
  if (!isOpen.value) open();
}

function arrowKey(dir: number) {
  openIfClosed();
  navigate(dir);
}

function moveToFirst() {
  openIfClosed();
  activeIndex.value = 0;
  focusOption(0);
}

function moveToLast() {
  openIfClosed();
  activeIndex.value = props.options.length - 1;
  focusOption(props.options.length - 1);
}

function handleCharKey(event: KeyboardEvent) {
  if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
    event.preventDefault();
    openIfClosed();
    handleType(event.key);
  }
}

function handleSelectKey(event: KeyboardEvent, source: "trigger" | "panel") {
  if (source === "panel" && (event.key === "Enter" || event.key === " ")) {
    event.preventDefault();
    selectOption(activeIndex.value);
    close();
  }
}

function handleCommonKeys(event: KeyboardEvent, source: "trigger" | "panel") {
  if (props.disabled) return;

  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      arrowKey(1);
      break;

    case "ArrowUp":
      event.preventDefault();
      arrowKey(-1);
      break;

    case "Home":
      event.preventDefault();
      moveToFirst();
      break;

    case "End":
      event.preventDefault();
      moveToLast();
      break;

    case "Escape":
      if (isOpen.value) {
        event.preventDefault();
        close();
      }
      break;

    case "Tab":
      if (isOpen.value) close();
      break;

    default:
      handleCharKey(event);
      break;
  }

  handleSelectKey(event, source);
}

function onTriggerKeydown(event: KeyboardEvent) {
  if (props.disabled) return;

  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    if (isOpen.value) {
      selectOption(activeIndex.value);
      close();
    } else {
      open();
    }
    return;
  }

  handleCommonKeys(event, "trigger");
}

function onPanelKeydown(event: KeyboardEvent) {
  handleCommonKeys(event, "panel");
}

function onOptionClick(index: number) {
  selectOption(index);
  close();
}

const stopClickOutside = onClickOutside(panelRef, (event) => {
  // Ignore clicks on the trigger so the toggle handler can do its work.
  if (triggerRef.value?.contains(event.target as Node)) return;
  close();
});

onMounted(() => {
  window.addEventListener("resize", updatePosition);
  window.addEventListener("scroll", updatePosition, true);
});

onUnmounted(() => {
  stopClickOutside();
  window.removeEventListener("resize", updatePosition);
  window.removeEventListener("scroll", updatePosition, true);
  if (typeAheadTimer) clearTimeout(typeAheadTimer);
});
</script>

<template>
  <div class="ui-select">
    <button
      ref="triggerRef"
      :id="id"
      :aria-label="ariaLabel"
      :aria-labelledby="ariaLabelledby"
      type="button"
      class="ui-select__trigger"
      :disabled="disabled"
      :aria-expanded="isOpen"
      aria-haspopup="listbox"
      :aria-controls="listboxId"
      :aria-activedescendant="activeDescendantId"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <span class="ui-select__label" :class="{ 'is-placeholder': !selectedOption }">
        {{ displayLabel }}
      </span>
      <span
        class="ui-select__chevron i-ri-arrow-down-s-line"
        :class="{ 'is-open': isOpen }"
        aria-hidden="true"
      />
    </button>

    <Teleport to="body">
      <Transition name="ui-select-panel">
        <div
          v-if="isOpen"
          :id="listboxId"
          ref="panelRef"
          role="listbox"
          class="ui-select__panel"
          :style="panelStyle"
          tabindex="-1"
          @keydown="onPanelKeydown"
        >
          <div
            v-for="(option, index) in props.options"
            :id="`${listboxId}-option-${index}`"
            :key="String(option.value)"
            :ref="(el) => setOptionRef(el, index)"
            role="option"
            class="ui-select__option"
            :class="{
              'is-selected': index === selectedIndex,
              'is-active': index === activeIndex && index !== selectedIndex,
            }"
            :aria-selected="index === selectedIndex"
            tabindex="-1"
            @click="onOptionClick(index)"
            @mouseenter="activeIndex = index"
          >
            {{ option.label }}
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.ui-select {
  position: relative;
  width: 100%;
}

.ui-select__trigger {
  width: 100%;
  min-height: 2.25rem;
  padding: 0 2.25rem 0 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text-main);
  font: inherit;
  text-align: left;
  cursor: pointer;
  display: flex;
  align-items: center;
  position: relative;
  transition:
    border-color var(--duration-fast) ease,
    box-shadow var(--duration-fast) ease,
    background-color var(--duration-fast) ease;
}

.ui-select__trigger:hover:not(:disabled) {
  border-color: var(--color-border-strong);
}

.ui-select__trigger:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.ui-select__trigger:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.ui-select__label {
  flex: 1 1 auto;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ui-select__label.is-placeholder {
  color: var(--color-text-muted);
}

.ui-select__chevron {
  position: absolute;
  right: 0.625rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-muted);
  transition: transform 0.2s ease;
  pointer-events: none;
}

.ui-select__chevron.is-open {
  transform: translateY(-50%) rotate(180deg);
}

.ui-select__panel {
  position: fixed;
  z-index: 1000;
  max-height: 16rem;
  overflow-y: auto;
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-card-hover);
  padding: 0.25rem 0;
}

.ui-select__option {
  padding: 0.5rem 0.75rem;
  font-size: var(--font-size-small);
  line-height: 1.4;
  cursor: pointer;
  color: var(--color-text-main);
  transition: background-color var(--duration-fast) ease;
  border-left: 2px solid transparent;
}

.ui-select__option:hover {
  background: var(--color-surface-hover);
}

.ui-select__option.is-active {
  background: var(--color-surface-muted);
}

.ui-select__option.is-selected {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
  border-left-color: var(--color-accent-strong);
}

.ui-select__option.is-selected:hover {
  background: var(--color-accent-soft);
}

.ui-select-panel-enter-active {
  transition:
    opacity 0.2s ease-out,
    transform 0.2s ease-out;
}

.ui-select-panel-leave-active {
  transition:
    opacity 0.15s ease-in,
    transform 0.15s ease-in;
}

.ui-select-panel-enter-from,
.ui-select-panel-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
