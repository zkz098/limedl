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
  <aside class="category-sidebar">
    <nav class="category-list" :aria-label="t('categories.all')">
      <button
        v-for="cat in categories"
        :key="cat.key"
        type="button"
        class="category-item"
        :class="{ 'category-item--active': activeCategory === cat.key }"
        @click="handleCategoryClick(cat.key)"
      >
        <span class="category-item__icon" :class="cat.icon" aria-hidden="true" />
        <span class="category-item__label">{{ t(cat.labelKey) }}</span>
        <span class="category-item__count">{{ counts[cat.key] ?? 0 }}</span>
      </button>
    </nav>

    <div class="category-sidebar__divider" aria-hidden="true" />

    <div class="category-sidebar__stats">
      <div class="stat-row">
        <span class="stat-row__label">{{ t("stats.totalTasks") }}</span>
        <span class="stat-row__value">{{ stats.totalTasks }}</span>
      </div>
      <div class="stat-row">
        <span class="stat-row__label">{{ t("stats.active") }}</span>
        <span class="stat-row__value">{{ stats.activeTasks }}</span>
      </div>
      <div class="stat-row">
        <span class="stat-row__label">{{ t("stats.completed") }}</span>
        <span class="stat-row__value">{{ stats.completedTasks }}</span>
      </div>
      <div class="stat-row">
        <span class="stat-row__label">{{ t("stats.currentSpeed") }}</span>
        <span class="stat-row__value">{{ formattedSpeed }}</span>
      </div>
    </div>

    <div class="category-sidebar__spacer" />

    <div class="category-sidebar__bottom">
      <div class="bottom-nav">
        <button
          v-for="nav in navItems"
          :key="nav.view"
          type="button"
          class="bottom-nav__item"
          :class="{ 'bottom-nav__item--active': currentView === nav.view }"
          :aria-label="t(`nav.${nav.view}`)"
          @click="handleNavigate(nav.view)"
        >
          <span class="bottom-nav__icon" :class="nav.icon" aria-hidden="true" />
        </button>
      </div>
      <div class="bottom-brand">
        <span class="i-ri-download-cloud-2-line bottom-brand__icon" aria-hidden="true" />
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

.category-list {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

.category-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-main);
  font: inherit;
  font-size: 0.875rem;
  cursor: pointer;
  text-align: left;
  transition: background-color 0.15s ease;
  position: relative;
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

.category-item__icon {
  font-size: 1rem;
  flex-shrink: 0;
  color: var(--color-text-muted);
}

.category-item--active .category-item__icon {
  color: var(--color-accent-strong);
}

.category-item__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.category-item__count {
  font-size: 0.75rem;
  color: var(--color-text-muted);
  background: var(--color-surface-muted);
  border-radius: var(--radius-pill);
  padding: 0 0.5rem;
  line-height: 1.4;
  flex-shrink: 0;
}

/* ── Divider ── */

.category-sidebar__divider {
  height: 1px;
  background: var(--color-border);
  margin: var(--space-2) 0;
}

/* ── Stats ── */

.category-sidebar__stats {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.stat-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  font-size: 0.8125rem;
  line-height: 1.4;
}

.stat-row__label {
  color: var(--color-text-muted);
}

.stat-row__value {
  color: var(--color-text-main);
  font-weight: 600;
  font-family: var(--font-mono);
}

/* ── Spacer ── */

.category-sidebar__spacer {
  flex: 1;
}

/* ── Bottom nav ── */

.category-sidebar__bottom {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding-top: var(--space-2);
}

.bottom-nav {
  display: flex;
  gap: var(--space-1);
}

.bottom-nav__item {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.25rem;
  height: 2.25rem;
  padding: var(--space-2);
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
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

.bottom-nav__icon {
  font-size: 1.25rem;
}

.bottom-brand {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  color: var(--color-text-muted);
  font-size: 0.75rem;
  font-weight: 600;
  opacity: 0.6;
}

.bottom-brand__icon {
  font-size: 0.875rem;
}
</style>
