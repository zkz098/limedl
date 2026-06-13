<script setup lang="ts">
import { ref } from "vue";

import { useI18n } from "../../i18n";

const { t } = useI18n();

const activeTab = ref("placeholder");

const tabs = [
  { id: "placeholder", icon: "i-ri-flask-line", labelKey: "labs.tabs.placeholder" },
] as const;
</script>

<template>
  <section class="labs-page">
    <div class="desk-panel__header">
      <div>
        <p class="section-kicker">{{ t("labs.kicker") }}</p>
        <h2 class="panel-title">{{ t("labs.title") }}</h2>
      </div>
    </div>

    <div class="labs-tabs">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="labs-tab"
        :class="{ 'labs-tab--active': activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        <span :class="tab.icon" aria-hidden="true" />
        <span>{{ t(tab.labelKey) }}</span>
      </button>
    </div>

    <div class="labs-tab-content">
      <section
        v-show="activeTab === 'placeholder'"
        class="settings-section"
      >
        <div class="settings-section__head">
          <div>
            <h3>{{ t("labs.comingSoon") }}</h3>
          </div>
          <span class="settings-section__icon i-ri-tools-line" aria-hidden="true" />
        </div>
        <p class="settings-section__summary">
          {{ t("labs.skeletonHint") }}
        </p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.labs-page {
  display: grid;
  gap: 1rem;
  padding-bottom: 3rem;
}

.labs-tabs {
  display: inline-grid;
  grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
  gap: 0.35rem;
  padding: 0.25rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--color-panel) var(--surface-panel-alpha), transparent);
}

.labs-tab {
  min-height: 2.25rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 0.82rem;
  padding: 0 0.6rem;
  transition:
    background-color 0.18s ease,
    border-color 0.18s ease,
    color 0.18s ease;
}

.labs-tab:hover {
  color: var(--color-heading);
  background: color-mix(in srgb, var(--color-accent-soft) 24%, var(--color-panel));
}

.labs-tab--active {
  color: var(--color-accent-strong);
  border-color: color-mix(in srgb, var(--color-accent) 18%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent-soft) 52%, var(--color-panel));
}

.labs-tab-content {
  display: grid;
  gap: 1rem;
}

@media (max-width: 960px) {
  .labs-tabs {
    grid-template-columns: repeat(auto-fit, minmax(5.5rem, 1fr));
  }

  .labs-tab {
    font-size: 0.78rem;
    padding: 0 0.35rem;
  }
}

@media (max-width: 680px) {
  .labs-tabs {
    display: flex;
    flex-wrap: nowrap;
    overflow-x: auto;
    scrollbar-width: none;
    -webkit-overflow-scrolling: touch;
  }

  .labs-tabs::-webkit-scrollbar {
    display: none;
  }

  .labs-tab {
    flex-shrink: 0;
    white-space: nowrap;
  }
}
</style>
