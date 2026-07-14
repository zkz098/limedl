<script setup lang="ts">
import { computed } from "vue";
import { t } from "../../i18n";
import { formatSpeed } from "../../lib/download-format";

const props = defineProps<{
  activeCategory: string;
  currentView: string;
  counts: Record<string, number>;
  stats: {
    totalTasks: number;
    activeTasks: number;
    completedTasks: number;
    currentSpeed: number;
  };
}>();

const emit = defineEmits<{
  "update:activeCategory": [category: string];
  navigate: [view: string];
}>();

const categories = [
  { key: "", icon: "i-ri-list-check", labelKey: "categories.all" },
  { key: "downloading", icon: "i-ri-download-line", labelKey: "categories.downloading" },
  { key: "paused", icon: "i-ri-pause-line", labelKey: "categories.paused" },
  { key: "completed", icon: "i-ri-check-line", labelKey: "categories.completed" },
  { key: "failed", icon: "i-ri-error-warning-line", labelKey: "categories.failed" },
  { key: "active", icon: "i-ri-flashlight-line", labelKey: "categories.active" },
];

const navItems = [
  { view: "home", icon: "i-ri-home-line" },
  { view: "settings", icon: "i-ri-settings-line" },
  { view: "labs", icon: "i-ri-flask-line" },
];

const formattedSpeed = computed(() => formatSpeed(props.stats.currentSpeed));

function handleCategoryClick(key: string) {
  emit("update:activeCategory", key);
}

function handleNavigate(view: string) {
  emit("navigate", view);
}
</script>

<template>
  <aside class="category-sidebar flex flex-col gap-1 p-3 overflow-y-auto">
    <nav class="category-list flex flex-col gap-[0.125rem]" :aria-label="t('categories.all')">
      <button
        v-for="cat in categories"
        :key="cat.key"
        type="button"
        class="category-item flex items-center gap-2 w-full px-3 py-2 border-none rounded-md bg-transparent text-sm cursor-pointer text-left relative"
        :class="{ 'category-item--active': activeCategory === cat.key }"
        @click="handleCategoryClick(cat.key)"
      >
        <span class="category-item__icon text-base flex-shrink-0" :class="cat.icon" aria-hidden="true" />
        <span class="category-item__label flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{{ t(cat.labelKey) }}</span>
        <span class="category-item__count text-xs rounded-full px-2 leading-[1.4] flex-shrink-0">{{ counts[cat.key] ?? 0 }}</span>
      </button>
    </nav>

    <div class="category-sidebar__divider h-px my-2" aria-hidden="true" />

    <div class="category-sidebar__stats flex flex-col gap-1">
      <div class="stat-row flex items-center justify-between gap-2 text-sm leading-[1.4]">
        <span class="stat-row__label">{{ t("stats.totalTasks") }}</span>
        <span class="stat-row__value font-semibold font-mono">{{ stats.totalTasks }}</span>
      </div>
      <div class="stat-row flex items-center justify-between gap-2 text-sm leading-[1.4]">
        <span class="stat-row__label">{{ t("stats.active") }}</span>
        <span class="stat-row__value font-semibold font-mono">{{ stats.activeTasks }}</span>
      </div>
      <div class="stat-row flex items-center justify-between gap-2 text-sm leading-[1.4]">
        <span class="stat-row__label">{{ t("stats.completed") }}</span>
        <span class="stat-row__value font-semibold font-mono">{{ stats.completedTasks }}</span>
      </div>
      <div class="stat-row flex items-center justify-between gap-2 text-sm leading-[1.4]">
        <span class="stat-row__label">{{ t("stats.currentSpeed") }}</span>
        <span class="stat-row__value font-semibold font-mono">{{ formattedSpeed }}</span>
      </div>
    </div>

    <div class="category-sidebar__spacer flex-1" />

    <div class="category-sidebar__bottom flex flex-col items-center gap-2 pt-2">
      <div class="bottom-nav flex gap-1">
        <button
          v-for="nav in navItems"
          :key="nav.view"
          type="button"
          class="bottom-nav__item flex items-center justify-center w-9 h-9 p-2 border-none rounded-md bg-transparent cursor-pointer"
          :class="{ 'bottom-nav__item--active': currentView === nav.view }"
          :aria-label="t(`nav.${nav.view}`)"
          @click="handleNavigate(nav.view)"
        >
          <span class="bottom-nav__icon text-xl" :class="nav.icon" aria-hidden="true" />
        </button>
      </div>
      <div class="bottom-brand flex items-center gap-[0.35rem] text-xs font-semibold opacity-60">
        <span class="i-ri-download-cloud-2-line bottom-brand__icon text-sm" aria-hidden="true" />
        <span class="bottom-brand__text">{{ t("common.appName") }}</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.category-sidebar {
  width: clamp(13rem, 16vw, 15rem);
  background: var(--color-panel);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  padding: var(--space-3);
  gap: var(--space-1);
  overflow-y: auto;
}

/* ── Category list ── */

.category-item {
  color: var(--color-text-main);
  font: inherit;
  transition: background-color 0.15s ease;
}

.category-item:hover {
  background: var(--color-surface-muted);
}

.category-item:focus-visible,
.bottom-nav__item:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-accent-border);
}

.category-item--active {
  background: var(--color-accent-soft);
  border-left: 3px solid var(--color-accent);
  padding-left: calc(var(--space-3) - 3px);
}

.category-item--active .category-item__icon {
  color: var(--color-accent-strong);
}

.category-item__icon {
  color: var(--color-text-muted);
}

.category-item__count {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
}

/* ── Divider ── */

.category-sidebar__divider {
  background: var(--color-border);
}

/* ── Stats ── */

.stat-row__label {
  color: var(--color-text-muted);
}

.stat-row__value {
  color: var(--color-text-main);
}

/* ── Spacer ── */

.bottom-nav__item {
  color: var(--color-text-muted);
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.bottom-nav__item:hover {
  background: var(--color-surface-muted);
  color: var(--color-text-main);
}

.bottom-nav__item--active {
  color: var(--color-accent-strong);
}

.bottom-brand {
  color: var(--color-text-muted);
}
</style>
