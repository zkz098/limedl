<script setup lang="ts">
import { computed, ref } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiBadge from "../ui/UiBadge.vue";
import SettingsSection from "../settings/SettingsSection.vue";
import SettingsField from "../settings/SettingsField.vue";
import type { AppSettings, MatchType, ReplacementMode, UrlRewriteRule } from "../../types/settings";

const draft = defineModel<AppSettings>("draft", { required: true });
const props = defineProps<{
  t: (key: string, params?: Record<string, unknown>) => string;
}>();

let uidCounter = 0;
const dragRuleIndex = ref<number | null>(null);
const dragTargetInfo = ref<{ ruleIndex: number; targetIndex: number } | null>(null);
const expandedRuleIds = ref<Set<string>>(new Set());
const testUrl = ref("");

function ensureUrlRewrite(): void {
  if (!draft.value.urlRewrite) {
    draft.value.urlRewrite = { enabled: false, rules: [] };
  }
  if (!Array.isArray(draft.value.urlRewrite.rules)) {
    draft.value.urlRewrite.rules = [];
  }
}

const urlRewriteEnabled = computed<boolean>({
  get: () => draft.value.urlRewrite?.enabled ?? false,
  set: (value: boolean) => {
    ensureUrlRewrite();
    draft.value.urlRewrite.enabled = value;
  },
});

const matchTypeOptions = computed<{ label: string; value: MatchType }[]>(() => [
  { label: props.t("settings.urlRewrite.matchTypeHost"), value: "host" },
  { label: props.t("settings.urlRewrite.matchTypePrefix"), value: "prefix" },
  { label: props.t("settings.urlRewrite.matchTypeRegex"), value: "regex" },
  { label: props.t("settings.urlRewrite.matchTypeWildcard"), value: "wildcard" },
]);

const replacementModeOptions = computed<{ label: string; value: ReplacementMode }[]>(() => [
  { label: props.t("settings.urlRewrite.modePrefixProxy"), value: "prefix_proxy" },
  { label: props.t("settings.urlRewrite.modeTemplate"), value: "template" },
]);

function toggleRuleExpanded(ruleId: string): void {
  if (expandedRuleIds.value.has(ruleId)) {
    expandedRuleIds.value.delete(ruleId);
  } else {
    expandedRuleIds.value.add(ruleId);
  }
}

function addCustomRule(): void {
  ensureUrlRewrite();
  const rules = draft.value.urlRewrite.rules;
  const newId = `rule-${Date.now()}-${++uidCounter}`;
  const newRule: UrlRewriteRule = {
    id: newId,
    name: props.t("settings.urlRewrite.ruleNamePlaceholder"),
    enabled: true,
    matchType: "host",
    pattern: "",
    replacementMode: "prefix_proxy",
    targets: [
      {
        urlTemplate: "",
        enabled: true,
        order: 0,
        _uid: ++uidCounter,
      },
    ],
    encodeUrl: true,
    fallbackToOriginal: true,
    order: rules.length,
    _uid: ++uidCounter,
  };
  rules.push(newRule);
  expandedRuleIds.value.add(newId);
  renumberRules();
}

function addPreset(presetKey: "github" | "huggingface" | "civitai"): void {
  ensureUrlRewrite();
  const rules = draft.value.urlRewrite.rules;
  let newRule: UrlRewriteRule;

  if (presetKey === "github") {
    const id = `preset-github-${Date.now()}`;
    newRule = {
      id,
      name: props.t("settings.urlRewrite.presetGithub"),
      enabled: true,
      matchType: "host",
      pattern: "*.github.com",
      replacementMode: "prefix_proxy",
      targets: [
        { urlTemplate: "https://ghproxy.net", enabled: true, order: 0, _uid: ++uidCounter },
        { urlTemplate: "https://mirror.ghproxy.cc", enabled: true, order: 1, _uid: ++uidCounter },
      ],
      encodeUrl: true,
      fallbackToOriginal: true,
      order: rules.length,
      _uid: ++uidCounter,
    };
  } else if (presetKey === "huggingface") {
    const id = `preset-hf-${Date.now()}`;
    newRule = {
      id,
      name: props.t("settings.urlRewrite.presetHuggingFace"),
      enabled: true,
      matchType: "regex",
      pattern: "^https://huggingface\\.co/(.*)",
      replacementMode: "template",
      targets: [
        { urlTemplate: "https://hf-mirror.com/$1", enabled: true, order: 0, _uid: ++uidCounter },
      ],
      encodeUrl: false,
      fallbackToOriginal: true,
      order: rules.length,
      _uid: ++uidCounter,
    };
  } else {
    const id = `preset-civitai-${Date.now()}`;
    newRule = {
      id,
      name: props.t("settings.urlRewrite.presetCivitai"),
      enabled: true,
      matchType: "host",
      pattern: "*.civitai.com",
      replacementMode: "prefix_proxy",
      targets: [
        { urlTemplate: "https://civitai.work", enabled: true, order: 0, _uid: ++uidCounter },
      ],
      encodeUrl: false,
      fallbackToOriginal: true,
      order: rules.length,
      _uid: ++uidCounter,
    };
  }

  rules.push(newRule);
  expandedRuleIds.value.add(newRule.id);
  renumberRules();
}

function removeRule(index: number): void {
  ensureUrlRewrite();
  const rules = draft.value.urlRewrite.rules;
  const removed = rules.splice(index, 1)[0];
  if (removed) {
    expandedRuleIds.value.delete(removed.id);
  }
  renumberRules();
}

function renumberRules(): void {
  ensureUrlRewrite();
  draft.value.urlRewrite.rules.forEach((rule, index) => {
    rule.order = index;
  });
}

function addTarget(rule: UrlRewriteRule): void {
  rule.targets.push({
    urlTemplate: "",
    enabled: true,
    order: rule.targets.length,
    _uid: ++uidCounter,
  });
  renumberTargets(rule);
}

function removeTarget(rule: UrlRewriteRule, targetIndex: number): void {
  rule.targets.splice(targetIndex, 1);
  renumberTargets(rule);
}

function renumberTargets(rule: UrlRewriteRule): void {
  rule.targets.forEach((target, index) => {
    target.order = index;
  });
}

// ── Drag and Drop (Rules) ──────────────────────────────────────────

function onRuleDragStart(index: number): void {
  dragRuleIndex.value = index;
}

function onRuleDragOver(event: DragEvent, index: number): void {
  event.preventDefault();
  if (dragRuleIndex.value === null || dragRuleIndex.value === index) return;
  ensureUrlRewrite();
  const rules = draft.value.urlRewrite.rules;
  const moved = rules.splice(dragRuleIndex.value, 1)[0];
  rules.splice(index, 0, moved);
  dragRuleIndex.value = index;
  renumberRules();
}

function onRuleDragEnd(): void {
  dragRuleIndex.value = null;
}

// ── Drag and Drop (Targets inside a Rule) ──────────────────────────

function onTargetDragStart(ruleIndex: number, targetIndex: number): void {
  dragTargetInfo.value = { ruleIndex, targetIndex };
}

function onTargetDragOver(event: DragEvent, ruleIndex: number, targetIndex: number): void {
  event.preventDefault();
  if (!dragTargetInfo.value) return;
  if (
    dragTargetInfo.value.ruleIndex !== ruleIndex ||
    dragTargetInfo.value.targetIndex === targetIndex
  ) {
    return;
  }
  ensureUrlRewrite();
  const rule = draft.value.urlRewrite.rules[ruleIndex];
  if (!rule) return;
  const moved = rule.targets.splice(dragTargetInfo.value.targetIndex, 1)[0];
  rule.targets.splice(targetIndex, 0, moved);
  dragTargetInfo.value.targetIndex = targetIndex;
  renumberTargets(rule);
}

function onTargetDragEnd(): void {
  dragTargetInfo.value = null;
}

// ── Live Test Sandbox Engine (Frontend Evaluation) ──────────────────

function wildcardMatchJs(pattern: string, text: string): boolean {
  const pChars = Array.from(pattern);
  const tChars = Array.from(text);
  let pIdx = 0;
  let tIdx = 0;
  let starIdx: number | null = null;
  let matchIdx = 0;

  while (tIdx < tChars.length) {
    if (pIdx < pChars.length && (pChars[pIdx] === "?" || pChars[pIdx] === tChars[tIdx])) {
      pIdx++;
      tIdx++;
    } else if (pIdx < pChars.length && pChars[pIdx] === "*") {
      starIdx = pIdx;
      pIdx++;
      matchIdx = tIdx;
    } else if (starIdx !== null) {
      pIdx = starIdx + 1;
      matchIdx++;
      tIdx = matchIdx;
    } else {
      return false;
    }
  }

  while (pIdx < pChars.length && pChars[pIdx] === "*") {
    pIdx++;
  }

  return pIdx === pChars.length;
}

function matchesRuleJs(urlStr: string, rule: UrlRewriteRule): boolean {
  if (!rule.enabled || !rule.pattern.trim()) return false;
  const pattern = rule.pattern.trim();

  switch (rule.matchType) {
    case "host": {
      try {
        const parsed = new URL(urlStr);
        const host = parsed.hostname.toLowerCase();
        const patLower = pattern.toLowerCase();
        if (patLower.startsWith("*.")) {
          const suffix = patLower.slice(2);
          return host === suffix || host.endsWith(patLower.slice(1));
        }
        if (patLower.includes("*") || patLower.includes("?")) {
          return wildcardMatchJs(patLower, host);
        }
        return host === patLower;
      } catch {
        return false;
      }
    }
    case "prefix":
      return urlStr.startsWith(pattern);
    case "regex":
      try {
        const re = new RegExp(pattern);
        return re.test(urlStr);
      } catch {
        return false;
      }
    case "wildcard":
      return wildcardMatchJs(pattern, urlStr);
    default:
      return false;
  }
}

const testResult = computed<{
  matchedRule: UrlRewriteRule | null;
  candidates: string[];
}>(() => {
  const url = testUrl.value.trim();
  if (!url || !draft.value.urlRewrite?.enabled) {
    return { matchedRule: null, candidates: [] };
  }

  const rules = (draft.value.urlRewrite.rules ?? [])
    .filter((r) => r.enabled)
    .toSorted((a, b) => a.order - b.order);

  for (const rule of rules) {
    if (matchesRuleJs(url, rule)) {
      const activeTargets = rule.targets
        .filter((t) => t.enabled && t.urlTemplate.trim())
        .toSorted((a, b) => a.order - b.order);

      if (activeTargets.length === 0) continue;

      const encodedUrl = rule.encodeUrl ? encodeURIComponent(url) : url;
      const candidates: string[] = [];

      for (const target of activeTargets) {
        let generated = "";
        if (rule.replacementMode === "prefix_proxy") {
          const base = target.urlTemplate.trim().replace(/\/+$/, "");
          generated = `${base}/${encodedUrl}`;
        } else {
          const template = target.urlTemplate.trim();
          if (rule.matchType === "regex") {
            try {
              const re = new RegExp(rule.pattern.trim(), "g");
              generated = url.replace(re, template);
            } catch {
              generated = template.replace("{url}", encodedUrl).replace("{raw_url}", url);
            }
          } else {
            generated = template.replace("{url}", encodedUrl).replace("{raw_url}", url);
          }
        }
        if (generated && !candidates.includes(generated)) {
          candidates.push(generated);
        }
      }

      if (rule.fallbackToOriginal && !candidates.includes(url)) {
        candidates.push(url);
      }

      return {
        matchedRule: rule,
        candidates,
      };
    }
  }

  return { matchedRule: null, candidates: [] };
});
</script>

<template>
  <SettingsSection
    :title="t('settings.urlRewrite.title')"
    icon="i-ri-links-line"
    :summary="t('settings.urlRewrite.description')"
  >
    <!-- Main Enable Switch -->
    <SettingsField
      :label="t('settings.urlRewrite.enableLabel')"
      :hint="t('settings.urlRewrite.enableDescription')"
    >
      <UiSwitch v-model="urlRewriteEnabled" :label="t('settings.urlRewrite.enableLabel')" />
    </SettingsField>

    <div v-show="draft.urlRewrite?.enabled" class="url-rewrite-panel__body">
      <!-- Actions Bar -->
      <div class="url-rewrite-panel__actions-bar">
        <div class="url-rewrite-panel__actions-left">
          <UiButton variant="primary" size="sm" icon="i-ri-add-line" @click="addCustomRule">
            {{ t("settings.urlRewrite.addRule") }}
          </UiButton>
        </div>
        <div class="url-rewrite-panel__presets">
          <span class="url-rewrite-panel__preset-label"
            >{{ t("settings.urlRewrite.importPreset") }}:</span
          >
          <UiButton
            variant="secondary"
            size="sm"
            icon="i-ri-github-line"
            @click="addPreset('github')"
          >
            GitHub
          </UiButton>
          <UiButton
            variant="secondary"
            size="sm"
            icon="i-ri-brain-line"
            @click="addPreset('huggingface')"
          >
            Hugging Face
          </UiButton>
          <UiButton
            variant="secondary"
            size="sm"
            icon="i-ri-image-line"
            @click="addPreset('civitai')"
          >
            Civitai
          </UiButton>
        </div>
      </div>

      <p class="settings-field__hint url-rewrite-panel__drag-hint">
        {{ t("settings.urlRewrite.dragHint") }}
      </p>

      <!-- Rules List -->
      <div
        v-if="!draft.urlRewrite?.rules || draft.urlRewrite.rules.length === 0"
        class="url-rewrite-panel__empty"
        role="status"
      >
        <span class="i-ri-information-line" aria-hidden="true" />
        <span>{{ t("settings.urlRewrite.emptyRules") }}</span>
      </div>

      <div v-else class="url-rewrite-panel__rules-list">
        <div
          v-for="(rule, rIdx) in draft.urlRewrite.rules"
          :key="rule._uid ?? rule.id ?? rIdx"
          class="url-rewrite-card"
          :class="{ 'url-rewrite-card--dragging': dragRuleIndex === rIdx }"
          draggable="true"
          @dragstart="onRuleDragStart(rIdx)"
          @dragover="onRuleDragOver($event, rIdx)"
          @drop="dragRuleIndex = null"
          @dragend="onRuleDragEnd"
        >
          <!-- Rule Header -->
          <div class="url-rewrite-card__header">
            <span
              class="url-rewrite-card__drag-handle i-ri-draggable"
              aria-hidden="true"
              :title="t('settings.urlRewrite.dragHint')"
            />
            <UiSwitch
              v-model="rule.enabled"
              class="url-rewrite-card__switch"
              :title="t('settings.urlRewrite.enableLabel')"
            />
            <div class="url-rewrite-card__title-wrap" @click="toggleRuleExpanded(rule.id)">
              <span class="url-rewrite-card__name">{{ rule.name || "Untitled Rule" }}</span>
              <UiBadge variant="neutral" size="sm">
                {{ rule.matchType.toUpperCase() }}
              </UiBadge>
              <span v-if="rule.pattern" class="url-rewrite-card__pattern-preview">
                {{ rule.pattern }}
              </span>
              <span class="url-rewrite-card__target-count">
                ({{ rule.targets.length }} targets)
              </span>
            </div>

            <div class="url-rewrite-card__header-actions">
              <UiButton
                variant="ghost"
                size="sm"
                :icon="
                  expandedRuleIds.has(rule.id) ? 'i-ri-arrow-up-s-line' : 'i-ri-arrow-down-s-line'
                "
                :aria-label="
                  expandedRuleIds.has(rule.id)
                    ? t('settings.urlRewrite.collapseRule')
                    : t('settings.urlRewrite.expandRule')
                "
                @click="toggleRuleExpanded(rule.id)"
              />
              <UiButton
                variant="ghost"
                size="sm"
                icon="i-ri-delete-bin-line"
                :aria-label="t('settings.urlRewrite.deleteRule')"
                @click="removeRule(rIdx)"
              />
            </div>
          </div>

          <!-- Expanded Rule Editor -->
          <div v-show="expandedRuleIds.has(rule.id)" class="url-rewrite-card__content">
            <div class="url-rewrite-card__grid">
              <div class="url-rewrite-card__field">
                <label class="url-rewrite-card__label">{{
                  t("settings.urlRewrite.ruleName")
                }}</label>
                <UiTextField
                  v-model="rule.name"
                  type="text"
                  :placeholder="t('settings.urlRewrite.ruleNamePlaceholder')"
                />
              </div>

              <div class="url-rewrite-card__field">
                <label class="url-rewrite-card__label">{{
                  t("settings.urlRewrite.matchType")
                }}</label>
                <UiSelect v-model="rule.matchType" :options="matchTypeOptions" />
              </div>
            </div>

            <div class="url-rewrite-card__grid">
              <div class="url-rewrite-card__field">
                <label class="url-rewrite-card__label">{{
                  t("settings.urlRewrite.pattern")
                }}</label>
                <UiTextField
                  v-model="rule.pattern"
                  type="text"
                  :placeholder="
                    rule.matchType === 'host'
                      ? t('settings.urlRewrite.patternPlaceholderHost')
                      : rule.matchType === 'prefix'
                        ? t('settings.urlRewrite.patternPlaceholderPrefix')
                        : rule.matchType === 'regex'
                          ? t('settings.urlRewrite.patternPlaceholderRegex')
                          : t('settings.urlRewrite.patternPlaceholderWildcard')
                  "
                />
              </div>

              <div class="url-rewrite-card__field">
                <label class="url-rewrite-card__label">{{
                  t("settings.urlRewrite.replacementMode")
                }}</label>
                <UiSelect v-model="rule.replacementMode" :options="replacementModeOptions" />
              </div>
            </div>

            <!-- Options Switches -->
            <div class="url-rewrite-card__options">
              <div class="url-rewrite-card__option-item">
                <UiSwitch v-model="rule.encodeUrl" :label="t('settings.urlRewrite.encodeUrl')" />
                <span class="url-rewrite-card__option-hint">{{
                  t("settings.urlRewrite.encodeUrlHint")
                }}</span>
              </div>
              <div class="url-rewrite-card__option-item">
                <UiSwitch
                  v-model="rule.fallbackToOriginal"
                  :label="t('settings.urlRewrite.fallbackToOriginal')"
                />
                <span class="url-rewrite-card__option-hint">{{
                  t("settings.urlRewrite.fallbackToOriginalHint")
                }}</span>
              </div>
            </div>

            <!-- Targets Section -->
            <div class="url-rewrite-card__targets-section">
              <div class="url-rewrite-card__targets-header">
                <span class="url-rewrite-card__targets-title">{{
                  t("settings.urlRewrite.targets")
                }}</span>
                <UiButton
                  variant="secondary"
                  size="sm"
                  icon="i-ri-add-line"
                  @click="addTarget(rule)"
                >
                  {{ t("settings.urlRewrite.addTarget") }}
                </UiButton>
              </div>

              <div
                v-if="rule.targets.length === 0"
                class="url-rewrite-panel__empty url-rewrite-card__empty-targets"
              >
                <span class="i-ri-information-line" aria-hidden="true" />
                <span>{{ t("settings.urlRewrite.emptyTargets") }}</span>
              </div>

              <ul v-else class="url-rewrite-card__targets-list">
                <li
                  v-for="(target, tIdx) in rule.targets"
                  :key="target._uid ?? tIdx"
                  class="url-rewrite-card__target-item"
                  :class="{
                    'url-rewrite-card__target-item--dragging':
                      dragTargetInfo?.ruleIndex === rIdx && dragTargetInfo?.targetIndex === tIdx,
                  }"
                  draggable="true"
                  @dragstart="onTargetDragStart(rIdx, tIdx)"
                  @dragover="onTargetDragOver($event, rIdx, tIdx)"
                  @drop="dragTargetInfo = null"
                  @dragend="onTargetDragEnd"
                >
                  <span
                    class="url-rewrite-panel__drag-handle i-ri-draggable"
                    aria-hidden="true"
                    :title="t('settings.urlRewrite.dragHint')"
                  />
                  <div class="url-rewrite-card__target-input">
                    <UiTextField
                      v-model="target.urlTemplate"
                      type="text"
                      :placeholder="
                        rule.replacementMode === 'prefix_proxy'
                          ? t('settings.urlRewrite.targetPlaceholder')
                          : t('settings.urlRewrite.targetTemplatePlaceholder')
                      "
                    />
                  </div>
                  <UiSwitch
                    v-model="target.enabled"
                    class="url-rewrite-card__target-switch"
                    :title="t('settings.urlRewrite.enableLabel')"
                  />
                  <UiButton
                    variant="ghost"
                    size="sm"
                    icon="i-ri-delete-bin-line"
                    :aria-label="t('settings.urlRewrite.deleteTarget')"
                    @click="removeTarget(rule, tIdx)"
                  />
                </li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      <!-- Live Test Sandbox -->
      <div class="url-rewrite-sandbox">
        <div class="url-rewrite-sandbox__header">
          <span class="i-ri-flask-line url-rewrite-sandbox__icon" aria-hidden="true" />
          <span class="url-rewrite-sandbox__title">{{ t("settings.urlRewrite.testSandbox") }}</span>
        </div>
        <div class="url-rewrite-sandbox__input-row">
          <UiTextField
            v-model="testUrl"
            type="url"
            inputmode="url"
            :placeholder="t('settings.urlRewrite.testUrlPlaceholder')"
          />
        </div>

        <div v-if="testUrl.trim()" class="url-rewrite-sandbox__result">
          <div v-if="testResult.matchedRule" class="url-rewrite-sandbox__matched">
            <div class="url-rewrite-sandbox__match-badge">
              <span class="i-ri-checkbox-circle-fill text-success" aria-hidden="true" />
              <span>{{
                t("settings.urlRewrite.testMatchedRule", { name: testResult.matchedRule.name })
              }}</span>
            </div>
            <p class="url-rewrite-sandbox__candidates-title">
              {{ t("settings.urlRewrite.testCandidatesTitle") }}
            </p>
            <ol class="url-rewrite-sandbox__candidates-list">
              <li
                v-for="(cand, cIdx) in testResult.candidates"
                :key="cIdx"
                class="url-rewrite-sandbox__candidate-item"
              >
                <span class="url-rewrite-sandbox__cand-idx">{{ cIdx + 1 }}.</span>
                <span class="url-rewrite-sandbox__cand-url">{{ cand }}</span>
                <UiBadge
                  v-if="
                    cIdx === testResult.candidates.length - 1 &&
                    testResult.matchedRule.fallbackToOriginal
                  "
                  variant="neutral"
                  size="sm"
                >
                  Fallback
                </UiBadge>
              </li>
            </ol>
          </div>
          <div v-else class="url-rewrite-sandbox__no-match">
            <span class="i-ri-information-line" aria-hidden="true" />
            <span>{{ t("settings.urlRewrite.testNoMatch") }}</span>
          </div>
        </div>
      </div>
    </div>
  </SettingsSection>
</template>

<style scoped>
.url-rewrite-panel__body {
  display: grid;
  gap: var(--space-4);
  margin-top: var(--space-4);
  padding-top: var(--space-4);
  border-top: 1px solid var(--color-border);
}

.url-rewrite-panel__actions-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.url-rewrite-panel__actions-left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.url-rewrite-panel__presets {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.url-rewrite-panel__preset-label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.url-rewrite-panel__drag-hint {
  margin: 0;
}

.url-rewrite-panel__empty {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  border: 1px dashed var(--color-border);
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}

.url-rewrite-panel__rules-list {
  display: grid;
  gap: var(--space-3);
}

.url-rewrite-card {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-muted);
  transition:
    background-color 0.15s ease,
    border-color 0.15s ease,
    box-shadow 0.15s ease;
  overflow: hidden;
}

.url-rewrite-card:hover {
  border-color: var(--color-border-strong);
}

.url-rewrite-card--dragging {
  opacity: 0.6;
  border-color: var(--color-accent-strong);
  box-shadow: var(--shadow-card);
}

.url-rewrite-card__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--color-surface-muted);
  border-bottom: 1px solid var(--color-border);
}

.url-rewrite-card__drag-handle {
  color: var(--color-text-muted);
  font-size: 1.1rem;
  cursor: grab;
  flex: 0 0 auto;
}

.url-rewrite-card__drag-handle:active {
  cursor: grabbing;
}

.url-rewrite-card__switch {
  flex: 0 0 auto;
}

.url-rewrite-card__title-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1 1 auto;
  min-width: 0;
  cursor: pointer;
}

.url-rewrite-card__name {
  font-weight: 500;
  color: var(--color-text);
  font-size: var(--font-size-base);
}

.url-rewrite-card__pattern-preview {
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.url-rewrite-card__target-count {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.url-rewrite-card__header-actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex: 0 0 auto;
}

.url-rewrite-card__content {
  padding: var(--space-4);
  display: grid;
  gap: var(--space-4);
  background: var(--color-panel-muted);
}

.url-rewrite-card__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

@media (max-width: 680px) {
  .url-rewrite-card__grid {
    grid-template-columns: 1fr;
  }
}

.url-rewrite-card__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.url-rewrite-card__label {
  font-size: var(--font-size-small);
  font-weight: 500;
  color: var(--color-text);
}

.url-rewrite-card__options {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--color-surface-muted);
  border-radius: var(--radius-md);
}

@media (max-width: 680px) {
  .url-rewrite-card__options {
    grid-template-columns: 1fr;
  }
}

.url-rewrite-card__option-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.url-rewrite-card__option-hint {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  line-height: 1.3;
}

.url-rewrite-card__targets-section {
  display: grid;
  gap: var(--space-2);
}

.url-rewrite-card__targets-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.url-rewrite-card__targets-title {
  font-size: var(--font-size-small);
  font-weight: 600;
  color: var(--color-text);
}

.url-rewrite-card__empty-targets {
  padding: var(--space-3);
}

.url-rewrite-card__targets-list {
  display: grid;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.url-rewrite-card__target-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel);
}

.url-rewrite-card__target-item:hover {
  border-color: var(--color-border-strong);
}

.url-rewrite-card__target-item--dragging {
  opacity: 0.6;
  border-color: var(--color-accent-strong);
}

.url-rewrite-panel__drag-handle {
  flex: 0 0 auto;
  color: var(--color-text-muted);
  font-size: 1.1rem;
  cursor: grab;
}

.url-rewrite-panel__drag-handle:active {
  cursor: grabbing;
}

.url-rewrite-card__target-input {
  flex: 1 1 auto;
  min-width: 0;
}

.url-rewrite-card__target-switch {
  flex: 0 0 auto;
}

/* ── Live Test Sandbox ─────────────────────────────────────────── */

.url-rewrite-sandbox {
  margin-top: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-surface-muted);
  display: grid;
  gap: var(--space-3);
}

.url-rewrite-sandbox__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.url-rewrite-sandbox__icon {
  font-size: 1.2rem;
  color: var(--color-accent);
}

.url-rewrite-sandbox__title {
  font-weight: 600;
  font-size: var(--font-size-base);
  color: var(--color-text);
}

.url-rewrite-sandbox__input-row {
  display: flex;
  gap: var(--space-2);
}

.url-rewrite-sandbox__result {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
}

.url-rewrite-sandbox__match-badge {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-weight: 500;
  color: var(--color-text);
  margin-bottom: var(--space-2);
}

.url-rewrite-sandbox__candidates-title {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
  margin: var(--space-2) 0 var(--space-1) 0;
}

.url-rewrite-sandbox__candidates-list {
  display: grid;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.url-rewrite-sandbox__candidate-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  word-break: break-all;
}

.url-rewrite-sandbox__cand-idx {
  color: var(--color-text-muted);
  font-weight: 600;
}

.url-rewrite-sandbox__cand-url {
  flex: 1 1 auto;
}

.url-rewrite-sandbox__no-match {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-text-muted);
  font-size: var(--font-size-small);
}
</style>
