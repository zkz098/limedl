import { ref, computed, type Ref } from "vue";
import { saveAppSettings } from "../lib/tauri/settings-api";
import type { AppSettings } from "../types/settings";

export interface SetupStep {
  id: string;
  labelKey: string;
  icon: string;
  skippable: boolean;
  order: number;
}

function cloneSettings<T>(settings: T): T {
  // structuredClone handles deep cloning of plain objects and primitives,
  // which includes Vue reactive proxies (the clone algorithm accesses
  // [[Get]] on each level, yielding unwrapped values).
  return structuredClone(settings);
}

/**
 * Fallback defaults matching Rust's `AppSettings::default()`.
 * When the Tauri backend is available, prefer passing the result of
 * `getAppSettings()` as `initialSettings` — Rust is the single source of truth.
 * Keep this function in sync with `src-tauri/src/download/types.rs` defaults.
 */
function createDefaultSettings(): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "lime",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: true,
      showHeatmap: true,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "traditional",
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 16,
        maxThreadsPerTask: 8,
        minThreadsPerTask: 0,
        adaptiveProfile: "balanced",
      },
      chunkSizeStrategy: "adaptive",
      tailSprintEnabled: false,
      connectionWarmupEnabled: true,
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 5,
      defaultChecksum: "blake3",
      defaultUserAgent:
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "https://cf.trackerslist.com/best.txt",
      listenPort: null,
      listenPortRange: null,
      upnpEnabled: false,
      enableNatpmp: true,
      enableIpv6: true,
      enablePex: true,
      enableLsd: true,
      enableUtp: true,
      enableFastExtension: true,
      enableHolepunch: true,
      enableWebSeed: true,
      enableSuperSeeding: false,
      globalDownloadRateLimit: 0,
      globalUploadRateLimit: 0,
      preallocateMode: "none",
      encryptionMode: "enabled",
      maxDownloads: 3,
      maxSeeds: 5,
      maxTorrents: 100,
      activeLimit: 500,
    },
    logging: {
      enabled: true,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: true, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: {
      bufferLimitMb: 1024,
      gameModeBufferMb: 128,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 4,
      gameModeMaxParallel: 1,
      hddBufferEnabled: true,
      ssdWriteCombineMb: 0,
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    doubleClick: { onCompleted: "none", onUncompleted: "none" },
    speedLimitSchedule: [],
  };
}

export function useSetupWizard(initialSettings?: AppSettings) {
  const currentStepIndex = ref(0);
  const isCompleted = ref(false);
  const isSaving = ref(false);
  const completedStepIndices = ref(new Set<number>());

  const settings = ref<AppSettings>(
    initialSettings ? cloneSettings(initialSettings) : createDefaultSettings(),
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
