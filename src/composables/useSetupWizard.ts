import { ref, computed, type Ref } from "vue";
import { saveAppSettings } from "../lib/tauri/settings-api";
import { DEFAULT_APP_SETTINGS } from "../lib/app-settings-defaults";
import type { AppSettings } from "../types/settings";

export interface SetupStep {
  id: string;
  labelKey: string;
  icon: string;
  skippable: boolean;
  order: number;
}

function cloneSettings<T>(settings: T): T {
  // Deep-clone via JSON round-trip instead of structuredClone: the latter
  // throws DataCloneError on Proxy objects — including Vue reactive proxies
  // (appSettings flows from a pinia store) and readonly component props.
  // AppSettings is plain JSON data, so the round-trip yields an equivalent
  // plain object.
  // NOSONAR: structuredClone throws DataCloneError on Vue reactive proxies —
  // JSON round-trip is intentional here.
  return JSON.parse(JSON.stringify(settings)); // NOSONAR
}

export function useSetupWizard(initialSettings?: AppSettings) {
  const currentStepIndex = ref(0);
  const isCompleted = ref(false);
  const isSaving = ref(false);
  const completedStepIndices = ref(new Set<number>());

  const settings = ref<AppSettings>(
    initialSettings ? cloneSettings(initialSettings) : cloneSettings(DEFAULT_APP_SETTINGS),
  );

  const steps: SetupStep[] = [
    {
      id: "welcome",
      labelKey: "setupWizard.stepWelcome",
      icon: "i-ri-home-smile-line",
      skippable: false,
      order: 0,
    },
    {
      id: "language",
      labelKey: "setupWizard.stepLanguage",
      icon: "i-ri-translate-2",
      skippable: true,
      order: 1,
    },
    {
      id: "appearance",
      labelKey: "setupWizard.stepAppearance",
      icon: "i-ri-palette-line",
      skippable: true,
      order: 2,
    },
    {
      id: "cdn",
      labelKey: "setupWizard.stepCdn",
      icon: "i-ri-flashlight-fill",
      skippable: true,
      order: 3,
    },
    {
      id: "rpc",
      labelKey: "setupWizard.stepRpc",
      icon: "i-ri-server-line",
      skippable: true,
      order: 4,
    },
    {
      id: "directory",
      labelKey: "setupWizard.stepDirectory",
      icon: "i-ri-folder-download-line",
      skippable: false,
      order: 5,
    },
    {
      id: "performance",
      labelKey: "setupWizard.stepPerformance",
      icon: "i-ri-speed-up-line",
      skippable: true,
      order: 6,
    },
    {
      id: "system",
      labelKey: "setupWizard.stepSystem",
      icon: "i-ri-settings-3-line",
      skippable: true,
      order: 7,
    },
    {
      id: "summary",
      labelKey: "setupWizard.stepSummary",
      icon: "i-ri-file-list-3-line",
      skippable: false,
      order: 8,
    },
  ];

  const currentStep = computed(() => steps[currentStepIndex.value]);

  function markStepsCompletedUpTo(index: number) {
    const next = new Set<number>();
    for (let i = 0; i <= index; i++) {
      next.add(i);
    }
    completedStepIndices.value = next;
  }

  function goToStep(index: number) {
    if (index < 0 || index >= steps.length) {
      return;
    }
    // Only allow jumping to completed steps or the current step; future steps are locked.
    if (index > currentStepIndex.value && !completedStepIndices.value.has(index)) {
      return;
    }
    currentStepIndex.value = index;
  }

  function nextStep() {
    if (currentStepIndex.value < steps.length - 1) {
      markStepsCompletedUpTo(currentStepIndex.value);
      currentStepIndex.value++;
    }
  }

  function previousStep() {
    if (currentStepIndex.value > 0) {
      currentStepIndex.value--;
    }
  }

  function skipStep() {
    const step = currentStep.value;
    if (!step?.skippable) {
      return;
    }
    nextStep();
  }

  async function completeWizard() {
    if (isSaving.value) {
      return;
    }
    isSaving.value = true;
    try {
      settings.value.setupCompleted = true;
      settings.value.lastSetupStep = steps.length - 1;
      await saveAppSettings(settings.value);
      isCompleted.value = true;
    } catch (error) {
      console.error("Failed to save setup settings", error);
    } finally {
      isSaving.value = false;
    }
  }

  function isCurrentStepSkippable() {
    return currentStep.value?.skippable ?? false;
  }

  function isFirstStep() {
    return currentStepIndex.value === 0;
  }

  function isLastStep() {
    return currentStepIndex.value === steps.length - 1;
  }

  return {
    currentStepIndex,
    isCompleted,
    isSaving,
    steps,
    settings: settings as Ref<AppSettings>,
    completedStepIndices,
    goToStep,
    nextStep,
    previousStep,
    skipStep,
    completeWizard,
    markStepsCompletedUpTo,
    isCurrentStepSkippable,
    isFirstStep,
    isLastStep,
  };
}
