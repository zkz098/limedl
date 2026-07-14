<script setup lang="ts">
import { ref } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiCard from "../ui/UiCard.vue";
import UiInput from "../ui/UiInput.vue";
import type { AppSettings } from "../../types/settings";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, params?: Record<string, unknown>) => string;
}>();

const dragIndex = ref<number | null>(null);
let uidCounter = 0;

function ensureGithubMirror(): void {
  if (!props.draft.githubMirror) {
    props.draft.githubMirror = { enabled: false, mirrors: [] };
  }
  if (!Array.isArray(props.draft.githubMirror.mirrors)) {
    props.draft.githubMirror.mirrors = [];
  }
}

function addMirror(): void {
  ensureGithubMirror();
  const mirrors = props.draft.githubMirror.mirrors;
  mirrors.push({
    url: "",
    enabled: true,
    order: mirrors.length,
    _uid: ++uidCounter,
  });
}

function removeMirror(index: number): void {
  ensureGithubMirror();
  const mirrors = props.draft.githubMirror.mirrors;
  mirrors.splice(index, 1);
  renumberMirrors();
}

function renumberMirrors(): void {
  ensureGithubMirror();
  props.draft.githubMirror.mirrors.forEach((mirror, index) => {
    mirror.order = index;
  });
}

function onDragStart(index: number): void {
  dragIndex.value = index;
}

function onDragOver(event: DragEvent, index: number): void {
  event.preventDefault();
  if (dragIndex.value === null || dragIndex.value === index) {
    return;
  }

  ensureGithubMirror();
  const mirrors = props.draft.githubMirror.mirrors;
  const moved = mirrors.splice(dragIndex.value, 1)[0];
  mirrors.splice(index, 0, moved);
  dragIndex.value = index;
  renumberMirrors();
}

function onDrop(event: DragEvent): void {
  event.preventDefault();
  dragIndex.value = null;
}

function onDragEnd(): void {
  dragIndex.value = null;
}
</script>

<template>
  <UiCard>
    <template #header>
      <div class="github-mirror-panel__header">
        <div>
          <p class="section-kicker">{{ t("settings.githubMirror.title") }}</p>
          <p class="panel-title">{{ t("settings.githubMirror.description") }}</p>
        </div>
      </div>
    </template>

    <div class="settings-section">
      <div class="settings-section__head">
        <div>
          <h3>{{ t("settings.githubMirror.title") }}</h3>
          <p class="settings-section__summary">
            {{ t("settings.githubMirror.enableDescription") }}
          </p>
        </div>
        <button
          type="button"
          class="settings-toggle"
          :class="{ 'settings-toggle--active': draft.githubMirror?.enabled }"
          :aria-pressed="draft.githubMirror?.enabled"
          @click="ensureGithubMirror(); draft.githubMirror.enabled = !draft.githubMirror.enabled"
        >
          <span
            class="settings-toggle__icon"
            :class="
              draft.githubMirror?.enabled
                ? 'i-ri-checkbox-circle-fill'
                : 'i-ri-checkbox-blank-circle-line'
            "
            aria-hidden="true"
          />
          <span class="settings-toggle__text">{{ t("settings.githubMirror.enableLabel") }}</span>
        </button>
      </div>
    </div>

    <div v-show="draft.githubMirror?.enabled" class="github-mirror-panel__list-section">
      <div class="github-mirror-panel__list-header">
        <p class="settings-field__label">{{ t("settings.githubMirror.mirrorUrl") }}</p>
        <UiButton variant="secondary" size="sm" icon="i-ri-add-line" @click="addMirror">
          {{ t("settings.githubMirror.addMirror") }}
        </UiButton>
      </div>

      <p class="settings-field__hint github-mirror-panel__drag-hint">
        {{ t("settings.githubMirror.dragHint") }}
      </p>

      <div
        v-if="draft.githubMirror?.mirrors.length === 0"
        class="github-mirror-panel__empty"
        role="status"
      >
        <span class="i-ri-information-line" aria-hidden="true" />
        <span>{{ t("settings.githubMirror.emptyHint") }}</span>
      </div>

      <ul v-else class="github-mirror-panel__list" role="list">
        <li
          v-for="(mirror, index) in draft.githubMirror.mirrors"
          :key="mirror._uid ?? index"
          class="github-mirror-panel__item"
          :class="{ 'github-mirror-panel__item--dragging': dragIndex === index }"
          draggable="true"
          @dragstart="onDragStart(index)"
          @dragover="onDragOver($event, index)"
          @drop="onDrop"
          @dragend="onDragEnd"
        >
          <span
            class="github-mirror-panel__drag-handle i-ri-draggable"
            aria-hidden="true"
            :title="t('settings.githubMirror.dragHint')"
          />
          <div class="github-mirror-panel__url">
            <UiInput
              v-model="mirror.url"
              type="url"
              inputmode="url"
              :placeholder="t('settings.githubMirror.mirrorUrlPlaceholder')"
            />
          </div>
          <label class="github-mirror-panel__switch" :title="t('settings.githubMirror.enableLabel')">
            <input v-model="mirror.enabled" type="checkbox" />
            <span class="github-mirror-panel__switch-track">
              <span class="github-mirror-panel__switch-thumb" />
            </span>
          </label>
          <UiButton
            variant="ghost"
            size="sm"
            icon="i-ri-delete-bin-line"
            :aria-label="t('settings.githubMirror.deleteMirror')"
            @click="removeMirror(index)"
          />
        </li>
      </ul>
    </div>
  </UiCard>
</template>

<style scoped>
.github-mirror-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.github-mirror-panel__list-section {
  display: grid;
  gap: 0.65rem;
  margin-top: var(--space-4);
  padding-top: var(--space-4);
  border-top: 1px solid var(--color-border);
}

.github-mirror-panel__list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.github-mirror-panel__drag-hint {
  margin: 0;
}

.github-mirror-panel__empty {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

.github-mirror-panel__list {
  display: grid;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.github-mirror-panel__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

.github-mirror-panel__item:hover {
  border-color: var(--color-border-strong);
  background: var(--color-surface-muted);
}

.github-mirror-panel__item--dragging {
  opacity: 0.6;
  border-color: var(--color-accent-strong);
  box-shadow: var(--shadow-card);
}

.github-mirror-panel__drag-handle {
  flex: 0 0 auto;
  color: var(--color-text-muted);
  font-size: 1.1rem;
  cursor: grab;
}

.github-mirror-panel__drag-handle:active {
  cursor: grabbing;
}

.github-mirror-panel__url {
  flex: 1 1 auto;
  min-width: 0;
}

.github-mirror-panel__switch {
  flex: 0 0 auto;
  position: relative;
  display: inline-flex;
  align-items: center;
  cursor: pointer;
}

.github-mirror-panel__switch input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}

.github-mirror-panel__switch-track {
  width: 2.25rem;
  height: 1.25rem;
  display: inline-block;
  border-radius: 999px;
  background: var(--color-border-strong);
  transition: background-color 0.2s ease;
  position: relative;
}

.github-mirror-panel__switch input:checked + .github-mirror-panel__switch-track {
  background: var(--color-accent-strong);
}

.github-mirror-panel__switch-thumb {
  position: absolute;
  top: 0.125rem;
  left: 0.125rem;
  width: 1rem;
  height: 1rem;
  border-radius: 999px;
  background: var(--color-panel);
  transition: transform 0.2s ease;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
}

.github-mirror-panel__switch input:checked + .github-mirror-panel__switch-track .github-mirror-panel__switch-thumb {
  transform: translateX(1rem);
}

.github-mirror-panel__switch input:focus-visible + .github-mirror-panel__switch-track {
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

@media (max-width: 680px) {
  .github-mirror-panel__item {
    flex-wrap: wrap;
  }

  .github-mirror-panel__url {
    flex: 1 1 100%;
    order: 1;
    min-width: 0;
  }

  .github-mirror-panel__drag-handle {
    order: 2;
  }

  .github-mirror-panel__switch {
    order: 3;
    margin-left: auto;
  }
}
</style>
