import { computed, type Ref } from "vue";
import { formatBytes, formatSpeed, formatTimestamp } from "../../lib/download-format";
import type { ChecksumMode } from "../../types/download";
import type {
  AdaptiveProfile,
  AppSettings,
  DeviceLearningMode,
} from "../../types/settings";

export interface SettingsOptionArrays {
  adaptiveProfileOptions: Ref<Array<{ label: string; value: AdaptiveProfile }>>;
  deviceModeOptions: Ref<Array<{ label: string; value: DeviceLearningMode }>>;
  checksumOptions: Ref<Array<{ label: string; value: ChecksumMode }>>;
  logLevelOptions: Ref<Array<{ label: string; value: string }>>;
}

export const DEFAULT_TRACKER_LIST_URL = "https://cf.trackerslist.com/best.txt";
export const DEFAULT_HTTP_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

function formatPercent(value: number) {
  return `${(value * 100).toFixed(value >= 0.1 ? 0 : 1)}%`;
}

export function useSettingsSummaries(
  draft: AppSettings,
  t: (key: string, options?: Record<string, unknown>) => string,
  opts: SettingsOptionArrays,
) {
  function profileLabel(profile: AdaptiveProfile) {
    return (
      opts.adaptiveProfileOptions.value.find((option) => option.value === profile)?.label ?? profile
    );
  }

  function deviceModeLabel(mode: DeviceLearningMode) {
    return opts.deviceModeOptions.value.find((option) => option.value === mode)?.label ?? mode;
  }

  function stabilityLabel(score: number) {
    if (score >= 0.88) {
      return t("settings.metrics.high");
    }
    if (score >= 0.68) {
      return t("settings.metrics.medium");
    }
    return t("settings.metrics.low");
  }

  const currentScene = computed(() => {
    return draft.networkLearning.scenes[0] ?? null;
  });

  const pageSummary = computed(() => {
    if (draft.scheduler.mode === "traditional") {
      return t("settings.summaries.traditional", {
        tasks: draft.scheduler.traditional.maxParallelTasks,
        deviceMode: deviceModeLabel(draft.networkLearning.deviceMode),
      });
    }

    return t("settings.summaries.automatic", {
      threads: draft.scheduler.automatic.maxParallelThreads,
      perTask: draft.scheduler.automatic.maxThreadsPerTask,
      profile: profileLabel(draft.scheduler.automatic.adaptiveProfile),
    });
  });

  const proxySummary = computed(() => {
    if (draft.proxy.mode === "disabled") {
      return t("settings.summaries.proxyDisabled");
    }

    if (draft.proxy.mode === "system") {
      return t("settings.summaries.proxySystem");
    }

    return draft.proxy.manualUrl.trim()
      ? t("settings.summaries.proxyManual", { url: draft.proxy.manualUrl.trim() })
      : t("settings.summaries.proxyManualEmpty");
  });

  const loggingSummary = computed(() => {
    const levelLabel =
      opts.logLevelOptions.value.find((option) => option.value === draft.logging.level)?.label ??
      draft.logging.level;
    const path = draft.logging.filePath.trim() || t("settings.loggingAutoPath");

    return draft.logging.enabled
      ? t("settings.summaries.loggingEnabled", { level: levelLabel, path })
      : t("settings.summaries.loggingDisabled");
  });

  const downloadSummary = computed(() => {
    const location = draft.download.defaultDownloadDir.trim() || t("settings.unsetDefaultPath");
    const checksumLabel =
      opts.checksumOptions.value.find((option) => option.value === draft.download.defaultChecksum)
        ?.label ?? draft.download.defaultChecksum;

    return t("settings.summaries.download", {
      location,
      retries: draft.download.defaultMaxRetries,
      checksum: checksumLabel,
      userAgent: draft.download.defaultUserAgent.trim() || DEFAULT_HTTP_USER_AGENT,
    });
  });

  const btUploadLimitMiB = computed(() => Math.round(draft.bt.uploadLimitBytes / 1024 / 1024));

  function setBtUploadLimitMiB(value: number | null) {
    draft.bt.uploadLimitBytes = Math.max(0, Math.trunc(value ?? 0)) * 1024 * 1024;
  }

  const trackerListEntries = computed(() =>
    draft.bt.trackerList
      .split(/\r?\n/)
      .map((tracker) => tracker.trim())
      .filter(Boolean),
  );

  const btSummary = computed(() => {
    const dhtLabel = draft.bt.dhtEnabled ? t("common.enabled") : t("common.disabled");
    const pexLabel = draft.bt.pexEnabled ? t("common.enabled") : t("common.disabled");
    const trackerCount = trackerListEntries.value.length;

    if (!draft.bt.pauseUploadWhenLimitReached) {
      return t("settings.summaries.btDisabled", {
        dht: dhtLabel,
        pex: pexLabel,
        trackers: trackerCount,
      });
    }

    const uploadLimit =
      draft.bt.uploadLimitBytes > 0 ? formatBytes(draft.bt.uploadLimitBytes) : t("common.disabled");
    const ratioLimit =
      draft.bt.uploadRatioLimit > 0
        ? `${draft.bt.uploadRatioLimit.toFixed(2)}x`
        : t("common.disabled");

    return t("settings.summaries.bt", {
      dht: dhtLabel,
      pex: pexLabel,
      trackers: trackerCount,
      uploadLimit,
      ratioLimit,
    });
  });

  const networkLearningSummary = computed(() => {
    const scene = currentScene.value;
    if (!scene) {
      return t("settings.summaries.noNetworkProfile");
    }

    if (draft.networkLearning.deviceMode === "mobile") {
      return t("settings.summaries.mobile", {
        deviceMode: deviceModeLabel(draft.networkLearning.deviceMode),
      });
    }

    if (!scene.learningEnabled) {
      return t("settings.summaries.learningPaused");
    }

    if (!scene.learnedMetrics) {
      return t("settings.summaries.noLearningSamples");
    }

    return t("settings.summaries.learning", {
      deviceMode: deviceModeLabel(draft.networkLearning.deviceMode),
      samples: scene.learnedMetrics.sampleCount,
      threads: scene.learnedMetrics.recommendedInitialThreads,
    });
  });

  const networkMetricsCards = computed(() => {
    const metrics = currentScene.value?.learnedMetrics;
    const learningOpen =
      draft.networkLearning.deviceMode !== "mobile" && currentScene.value?.learningEnabled;

    return [
      {
        label: t("settings.metrics.learningStatus"),
        value: learningOpen ? t("common.enabled") : t("common.disabled"),
      },
      {
        label: t("settings.metrics.estimatedBandwidth"),
        value: metrics ? formatSpeed(metrics.estimatedBandwidthBps) : t("common.dash"),
      },
      {
        label: t("settings.metrics.stability"),
        value: metrics ? stabilityLabel(metrics.stabilityScore) : t("common.dash"),
      },
      {
        label: t("settings.metrics.penaltyRate"),
        value: metrics ? formatPercent(metrics.penaltyRate) : t("common.dash"),
      },
      {
        label: t("settings.metrics.recommendedInitialThreads"),
        value: metrics ? String(metrics.recommendedInitialThreads) : t("common.dash"),
      },
      {
        label: t("settings.metrics.recommendedThreadCap"),
        value: metrics ? String(metrics.recommendedMaxThreadsPerTaskCap) : t("common.dash"),
      },
      {
        label: t("settings.metrics.sampleCount"),
        value: metrics ? String(metrics.sampleCount) : "0",
      },
      {
        label: t("settings.metrics.lastLearnedAt"),
        value: metrics ? formatTimestamp(metrics.lastObservedAtMs) : t("common.dash"),
      },
    ];
  });

  return {
    currentScene,
    pageSummary,
    proxySummary,
    loggingSummary,
    downloadSummary,
    btUploadLimitMiB,
    setBtUploadLimitMiB,
    trackerListEntries,
    btSummary,
    networkLearningSummary,
    networkMetricsCards,
    deviceModeLabel,
    profileLabel,
  };
}
