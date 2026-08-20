<script setup lang="ts">
import type { AppSettings } from "../../types/settings";
import SettingsSection from "./SettingsSection.vue";
import SettingsField from "./SettingsField.vue";
import UiSwitch from "../ui/UiSwitch.vue";

defineProps<{
  t: (key: string) => string;
}>();

const draft = defineModel<AppSettings>("draft", { required: true });
</script>

<template>
  <div class="settings-panel flex flex-col gap-5">
    <SettingsSection
      title="桌宠"
      icon="i-ri-bear-smile-line"
      summary="桌面宠物设置（占位，后续可换模型）"
    >
      <div class="settings-grid">
        <SettingsField label="启用桌宠" hint="在桌面显示独立透明宠物窗口">
          <UiSwitch v-model="draft.pet.enabled" />
        </SettingsField>

        <SettingsField label="主窗口隐藏时保持显示" hint="关闭主窗口到托盘时，宠物是否常驻">
          <UiSwitch v-model="draft.pet.keepAliveWhenMainHidden" :disabled="!draft.pet.enabled" />
        </SettingsField>

        <SettingsField label="大小" :hint="`当前: ${draft.pet.scale.toFixed(1)}x`">
          <input
            v-model.number="draft.pet.scale"
            type="range"
            min="0.5"
            max="2"
            step="0.1"
            :disabled="!draft.pet.enabled"
            class="settings-range"
          />
        </SettingsField>

        <SettingsField label="不透明度" :hint="`当前: ${Math.round(draft.pet.opacity * 100)}%`">
          <input
            v-model.number="draft.pet.opacity"
            type="range"
            min="0.2"
            max="1"
            step="0.05"
            :disabled="!draft.pet.enabled"
            class="settings-range"
          />
        </SettingsField>

        <SettingsField
          label="透明背景"
          hint="开启后隐藏白色卡片背景，只显示角色本身（适合 Live2D/透明贴图）"
        >
          <UiSwitch v-model="draft.pet.transparentBackground" :disabled="!draft.pet.enabled" />
        </SettingsField>

        <SettingsField label="模型" hint="占位：后续支持多模型/换装">
          <select v-model="draft.pet.model" :disabled="!draft.pet.enabled" class="settings-select">
            <option value="default">default（占位）</option>
          </select>
        </SettingsField>
      </div>

      <div class="settings-hint">
        <p class="settings-field__hint">
          提示：可拖拽宠物移动位置；将链接拖到宠物上可快速新建下载；宠物会根据下载状态切换动画（待机/工作中/庆祝/沮丧）。
        </p>
      </div>
    </SettingsSection>
  </div>
</template>

<style scoped>
.settings-range {
  width: 100%;
}

.settings-select {
  width: 100%;
  padding: 0.5rem 0.75rem;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-input-bg);
  color: var(--color-text-main);
}

.settings-hint {
  margin-top: var(--space-2);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
}
</style>
