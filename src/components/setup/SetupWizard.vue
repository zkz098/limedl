<script setup lang="ts">
import { computed, onMounted, type Component } from "vue";
import { onKeyStroke } from "@vueuse/core";
import logoUrl from "../../assets/logo.png";

import { useI18n } from "../../i18n";
import { useSetupWizard } from "../../composables/useSetupWizard";
import { saveAppSettings } from "../../lib/tauri/settings-api";
import type { AppSettings } from "../../types/settings";
import UiButton from "../ui/UiButton.vue";
import SetupStepIndicator from "./SetupStepIndicator.vue";
import StepWelcome from "./steps/StepWelcome.vue";
import StepLanguage from "./steps/StepLanguage.vue";
import StepAppearance from "./steps/StepAppearance.vue";
import StepCdn from "./steps/StepCdn.vue";
import StepRpc from "./steps/StepRpc.vue";
import StepDirectory from "./steps/StepDirectory.vue";
import StepPerformance from "./steps/StepPerformance.vue";
import StepSystem from "./steps/StepSystem.vue";
import StepSummary from "./steps/StepSummary.vue";

const props = withDefaults(
  defineProps<{
    appName?: string;
    appVersion?: string;
    initialSettings?: AppSettings;
    startFromStep?: number;
  }>(),
  {
    appName: "Limedl",
    appVersion: "",
    startFromStep: 0,
  },
);

const emit = defineEmits<{
  close: [];
  completed: [settings: AppSettings];
}>();

const { t } = useI18n();

const wizard = useSetupWizard(props.initialSettings);

const stepComponents: Record<string, Component> = {
  welcome: StepWelcome,
  language: StepLanguage,
  appearance: StepAppearance,
  cdn: StepCdn,
  rpc: StepRpc,
  directory: StepDirectory,
  performance: StepPerformance,
  system: StepSystem,
  summary: StepSummary,
};

const currentStepComponent = computed(
  () => stepComponents[wizard.steps[wizard.currentStepIndex.value]?.id] ?? StepWelcome,
);

const nextButtonLabel = computed(() => {
  if (wizard.isFirstStep()) {
    return t("setupWizard.startButton");
  }
  if (wizard.isLastStep()) {
    return t("setupWizard.completeButton");
  }
  return t("setupWizard.nextButton");
});

const nextButtonIcon = computed(() => {
  if (wizard.isLastStep()) {
    return "i-ri-check-line";
  }
  return "i-ri-arrow-right-s-line";
});

async function handlePrimaryAction() {
  if (wizard.isLastStep()) {
    await wizard.completeWizard();
    emit("completed", wizard.settings.value);
    return;
  }
  wizard.nextStep();
}

async function handleSkipAll() {
  await wizard.completeWizard();
  emit("completed", wizard.settings.value);
}

function handleClose() {
  // Save current step for interruption recovery if wizard not fully completed
  if (!wizard.isCompleted.value) {
    const stepIndex = wizard.currentStepIndex.value;
    wizard.settings.value.lastSetupStep = stepIndex;
    // Fire-and-forget: save interruption state, don't block closing
    saveAppSettings(wizard.settings.value).catch((err) =>
      console.error("Failed to save setup interruption state:", err),
    );
  }
  emit("close");
}

function handleStepSelect(index: number) {
  wizard.goToStep(index);
}

onMounted(() => {
  // Jump to saved step for interruption recovery
  if (props.startFromStep != null && props.startFromStep > 0) {
    // Bypass goToStep guard (which blocks forward navigation to non-completed steps)
    // Recovery is an explicit re-entry, not a forward navigation
    wizard.markStepsCompletedUpTo(props.startFromStep - 1);
    wizard.currentStepIndex.value = props.startFromStep;
  }
});

onKeyStroke("Escape", () => {
  handleClose();
});
</script>

<template>
  <div
    class="setup-wizard"
    role="dialog"
    aria-modal="true"
    :aria-label="t('setupWizard.ariaLabel')"
  >
    <div class="setup-wizard__overlay" />
    <div class="setup-wizard__panel">
      <SetupStepIndicator
        :steps="wizard.steps"
        :current-step-index="wizard.currentStepIndex.value"
        :completed-step-indices="wizard.completedStepIndices.value"
        @select-step="handleStepSelect"
      />
      <div class="setup-wizard__body">
        <header class="setup-wizard__header">
          <div class="setup-wizard__brand">
            <img :src="logoUrl" class="setup-wizard__brand-logo" alt="Limedl" />
            <span class="setup-wizard__brand-name">{{ appName }}</span>
            <span class="setup-wizard__brand-version">{{ appVersion }}</span>
          </div>
        </header>
        <main class="setup-wizard__content">
          <Transition name="step-fade" mode="out-in">
            <component
              :is="currentStepComponent"
              :key="wizard.currentStepIndex.value"
              :settings="wizard.settings.value"
              @update:settings="wizard.settings.value = $event"
              @edit-step="wizard.goToStep($event)"
            />
          </Transition>
        </main>
        <footer class="setup-wizard__footer">
          <div class="setup-wizard__footer-left">
            <UiButton
              v-if="!wizard.isFirstStep()"
              variant="secondary"
              icon="i-ri-arrow-left-s-line"
              @click="wizard.previousStep()"
            >
              {{ t("setupWizard.backButton") }}
            </UiButton>
          </div>
          <div class="setup-wizard__footer-right">
            <UiButton v-if="wizard.isFirstStep()" variant="ghost" @click="handleSkipAll()">
              {{ t("setupWizard.skipAllButton") }}
            </UiButton>
            <UiButton
              v-if="wizard.isCurrentStepSkippable() && !wizard.isFirstStep()"
              variant="ghost"
              @click="wizard.skipStep()"
            >
              {{ t("setupWizard.skipButton") }}
            </UiButton>
            <UiButton
              variant="primary"
              :icon-right="nextButtonIcon"
              :loading="wizard.isSaving.value"
              @click="handlePrimaryAction()"
            >
              {{ nextButtonLabel }}
            </UiButton>
          </div>
        </footer>
      </div>
    </div>
  </div>
</template>

<style scoped>
.setup-wizard {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
}

.setup-wizard__overlay {
  position: absolute;
  inset: 0;
  background: var(--surface-overlay-bg);
  backdrop-filter: blur(var(--surface-blur));
  animation: wizard-overlay-enter 300ms ease-out;
}

.setup-wizard__panel {
  position: relative;
  display: flex;
  width: 100vw;
  height: 100vh;
  background: var(--color-bg-base);
  overflow: hidden;
  animation: wizard-panel-enter 300ms ease-out;
}

.setup-wizard__body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.setup-wizard__header {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: var(--space-4) var(--space-6);
  border-bottom: var(--border-width-thin) solid var(--color-border);
}

.setup-wizard__brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.setup-wizard__brand-logo {
  width: 1.25rem;
  height: 1.25rem;
  border-radius: var(--radius-sm);
}

.setup-wizard__brand-name {
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-main);
}

.setup-wizard__brand-version {
  opacity: 0.7;
}

.setup-wizard__content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  display: flex;
  flex-direction: column;
  padding: var(--space-4) 0;
}

.setup-wizard__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-6);
  border-top: var(--border-width-thin) solid var(--color-border);
}

.setup-wizard__footer-left,
.setup-wizard__footer-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

/* ── Lightweight SettingsSection for wizard steps ── */
.setup-wizard .settings-section {
  padding: var(--space-4);
  box-shadow: none;
  border-color: var(--color-border);
}

.setup-wizard .settings-section:hover {
  border-color: var(--color-border);
  box-shadow: none;
}

/* ── Step transitions ── */
.step-fade-enter-from {
  opacity: 0;
  transform: translateX(1.25rem);
}

.step-fade-enter-to {
  opacity: 1;
  transform: translateX(0);
}

.step-fade-leave-from {
  opacity: 1;
  transform: translateX(0);
}

.step-fade-leave-to {
  opacity: 0;
  transform: translateX(-1.25rem);
}

.step-fade-enter-active,
.step-fade-leave-active {
  transition:
    opacity 250ms ease-out,
    transform 250ms ease-out;
}

/* ── Wizard entrance animation ── */
@keyframes wizard-overlay-enter {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes wizard-panel-enter {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .setup-wizard__overlay,
  .setup-wizard__panel {
    animation: none;
  }

  .step-fade-enter-from,
  .step-fade-leave-to {
    opacity: 0;
    transform: none;
  }

  .step-fade-enter-active,
  .step-fade-leave-active {
    transition: opacity 150ms ease-out;
  }
}
</style>
