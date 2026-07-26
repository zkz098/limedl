import { computed, reactive, ref, watch, type Ref } from "vue";

import type { AppSettings } from "../../types/settings";
import { serializeSettings, settingsDraftSnapshot } from "./settingsUtils";
import { DEFAULT_HTTP_USER_AGENT, DEFAULT_TRACKER_LIST_URL } from "./useSettingsSummaries";

interface UseSettingsFormOptions {
  settings: Ref<AppSettings | null>;
  onDirtyChange?: (isDirty: boolean) => void;
}

/**
 * Shared settings form composable — eliminates ~180 lines of duplicated form
 * initialization, settings-sync watcher, dirty-tracking watcher, and payload
 * builder between SettingsPage.vue and LabsPage.vue.
 */
export function useSettingsForm(options: UseSettingsFormOptions) {
  const { settings, onDirtyChange } = options;

  // ── Reactive form ─────────────────────────────────────────────────

  const form = reactive<AppSettings>({
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
    proxy: {
      mode: "disabled",
      manualUrl: "",
    },
    scheduler: {
      mode: "automatic",
      traditional: {
        maxParallelTasks: 3,
      },
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
      defaultUserAgent: DEFAULT_HTTP_USER_AGENT,
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: DEFAULT_TRACKER_LIST_URL,
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
      preallocateMode: "none",
      encryptionMode: "enabled",
      maxDownloads: 3,
      maxSeeds: 5,
      maxTorrents: 100,
      activeLimit: 500,
      globalDownloadRateLimit: 0,
      globalUploadRateLimit: 0,
    },
    logging: {
      enabled: true,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: {
      enabled: true,
      port: 6800,
      secret: null,
      corsAllowedOrigins: [],
    },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: {
      enabled: false,
      mirrors: [],
    },
    notifications: {
      enabled: false,
    },
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
    doubleClick: {
      onCompleted: "none",
      onUncompleted: "none",
    },
    speedLimitSchedule: [],
  });

  const savedSettingsSnapshot = ref("");

  // ── Helpers ───────────────────────────────────────────────────────

  function buildSettingsPayload(): AppSettings {
    return {
      globalSpeedLimitBps: form.globalSpeedLimitBps,
      appearance: {
        themeColor: form.appearance.themeColor,
        backgroundOpacity: form.appearance.backgroundOpacity,
        colorMode: form.appearance.colorMode,
        showDetailInfo: form.appearance.showDetailInfo,
        showHeatmap: form.appearance.showHeatmap,
        sortKey: form.appearance.sortKey,
        sortDirection: form.appearance.sortDirection,
        compactView: form.appearance.compactView,
        visibleColumns: form.appearance.visibleColumns,
        closeBehavior: form.appearance.closeBehavior,
      },
      proxy: {
        mode: form.proxy.mode,
        manualUrl: form.proxy.manualUrl,
      },
      scheduler: {
        mode: form.scheduler.mode,
        traditional: {
          maxParallelTasks: form.scheduler.traditional.maxParallelTasks,
        },
        automatic: {
          maxParallelThreads: form.scheduler.automatic.maxParallelThreads,
          maxThreadsPerTask: form.scheduler.automatic.maxThreadsPerTask,
          minThreadsPerTask: form.scheduler.automatic.minThreadsPerTask,
          adaptiveProfile: form.scheduler.automatic.adaptiveProfile,
        },
        chunkSizeStrategy: form.scheduler.chunkSizeStrategy,
        tailSprintEnabled: form.scheduler.tailSprintEnabled,
        connectionWarmupEnabled: form.scheduler.connectionWarmupEnabled,
      },
      download: {
        defaultDownloadDir: form.download.defaultDownloadDir,
        defaultMaxRetries: form.download.defaultMaxRetries,
        defaultChecksum: form.download.defaultChecksum,
        defaultUserAgent: form.download.defaultUserAgent,
      },
      bt: {
        pauseUploadWhenLimitReached: form.bt.pauseUploadWhenLimitReached,
        uploadLimitBytes: form.bt.uploadLimitBytes,
        uploadRatioLimit: form.bt.uploadRatioLimit,
        dhtEnabled: form.bt.dhtEnabled,
        trackerList: form.bt.trackerList,
        trackerListUrl: form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL,
        listenPort: form.bt.listenPort ?? null,
        listenPortRange: form.bt.listenPortRange,
        upnpEnabled: form.bt.upnpEnabled,
        enableNatpmp: form.bt.enableNatpmp,
        enableIpv6: form.bt.enableIpv6,
        enablePex: form.bt.enablePex,
        enableLsd: form.bt.enableLsd,
        enableUtp: form.bt.enableUtp,
        enableFastExtension: form.bt.enableFastExtension,
        enableHolepunch: form.bt.enableHolepunch,
        enableWebSeed: form.bt.enableWebSeed,
        enableSuperSeeding: form.bt.enableSuperSeeding,
        preallocateMode: form.bt.preallocateMode,
        encryptionMode: form.bt.encryptionMode,
        maxDownloads: form.bt.maxDownloads,
        maxSeeds: form.bt.maxSeeds,
        maxTorrents: form.bt.maxTorrents,
        activeLimit: form.bt.activeLimit,
        globalDownloadRateLimit: form.bt.globalDownloadRateLimit,
        globalUploadRateLimit: form.bt.globalUploadRateLimit,
      },
      logging: {
        enabled: form.logging.enabled,
        level: form.logging.level,
        filePath: form.logging.filePath.trim(),
        retentionCount: form.logging.retentionCount ?? null,
        retentionDays: form.logging.retentionDays ?? null,
      },
      aria2Rpc: {
        enabled: form.aria2Rpc.enabled,
        port: form.aria2Rpc.port,
        secret: form.aria2Rpc.secret?.trim() || null,
        corsAllowedOrigins: form.aria2Rpc.corsAllowedOrigins ?? [],
      },
      cdnAcceleration: {
        ...form.cdnAcceleration,
      },
      githubMirror: {
        enabled: form.githubMirror?.enabled ?? false,
        mirrors: form.githubMirror?.mirrors?.map((mirror) => ({ ...mirror })) ?? [],
      },
      notifications: {
        enabled: form.notifications?.enabled ?? false,
      },
      ioBaseline: {
        bufferLimitMb: Math.max(64, Math.min(32768, form.ioBaseline.bufferLimitMb ?? 1024)),
        gameModeBufferMb: Math.max(16, Math.min(4096, form.ioBaseline.gameModeBufferMb ?? 128)),
        diskTypeOverrides: { ...form.ioBaseline.diskTypeOverrides },
        maxParallelHdd: Math.max(1, Math.min(16, form.ioBaseline.maxParallelHdd ?? 4)),
        gameModeMaxParallel: Math.max(1, Math.min(8, form.ioBaseline.gameModeMaxParallel ?? 1)),
        hddBufferEnabled: form.ioBaseline.hddBufferEnabled ?? true,
        ssdWriteCombineMb: form.ioBaseline.ssdWriteCombineMb ?? 0,
      },
      autostart: form.autostart ?? false,
      setupCompleted: form.setupCompleted ?? false,
      lastSetupStep: form.lastSetupStep ?? null,
      maxInMemoryDownloads: form.maxInMemoryDownloads ?? 200,
      doubleClick: {
        onCompleted: form.doubleClick?.onCompleted ?? "none",
        onUncompleted: form.doubleClick?.onUncompleted ?? "none",
      },
      speedLimitSchedule: form.speedLimitSchedule?.slice() ?? [],
    };
  }

  // ── Settings sync watcher ─────────────────────────────────────────

  const settingsDraftSnapshotComputed = computed(() =>
    settingsDraftSnapshot(buildSettingsPayload()),
  );

  watch(
    settings,
    (nextSettings) => {
      if (!nextSettings) {
        return;
      }

      form.globalSpeedLimitBps = nextSettings.globalSpeedLimitBps ?? 0;
      form.appearance.themeColor = nextSettings.appearance?.themeColor ?? "lime";
      form.appearance.backgroundOpacity = nextSettings.appearance?.backgroundOpacity ?? "default";
      form.appearance.colorMode = nextSettings.appearance?.colorMode ?? "system";
      form.appearance.showDetailInfo = nextSettings.appearance?.showDetailInfo ?? true;
      form.appearance.showHeatmap = nextSettings.appearance?.showHeatmap ?? true;
      form.appearance.sortKey = nextSettings.appearance?.sortKey ?? "added_at";
      form.appearance.sortDirection = nextSettings.appearance?.sortDirection ?? "desc";
      form.appearance.compactView = nextSettings.appearance?.compactView ?? false;
      form.appearance.visibleColumns = nextSettings.appearance?.visibleColumns ?? [
        "file",
        "size",
        "downloaded",
        "status",
        "progress",
        "speed",
        "eta",
      ];
      form.proxy.mode = nextSettings.proxy.mode;
      form.proxy.manualUrl = nextSettings.proxy.manualUrl;
      form.scheduler.mode = nextSettings.scheduler.mode;
      form.scheduler.traditional.maxParallelTasks =
        nextSettings.scheduler.traditional.maxParallelTasks;
      form.scheduler.automatic.maxParallelThreads =
        nextSettings.scheduler.automatic.maxParallelThreads;
      form.scheduler.automatic.maxThreadsPerTask =
        nextSettings.scheduler.automatic.maxThreadsPerTask;
      form.scheduler.automatic.minThreadsPerTask =
        nextSettings.scheduler.automatic.minThreadsPerTask ?? 0;
      form.scheduler.automatic.adaptiveProfile = nextSettings.scheduler.automatic.adaptiveProfile;
      form.scheduler.chunkSizeStrategy = nextSettings.scheduler.chunkSizeStrategy ?? "adaptive";
      form.download.defaultDownloadDir = nextSettings.download.defaultDownloadDir;
      form.download.defaultMaxRetries = nextSettings.download.defaultMaxRetries;
      form.download.defaultChecksum = nextSettings.download.defaultChecksum;
      form.download.defaultUserAgent =
        nextSettings.download.defaultUserAgent || DEFAULT_HTTP_USER_AGENT;
      form.bt.dhtEnabled = nextSettings.bt.dhtEnabled;
      form.bt.trackerList = nextSettings.bt.trackerList;
      form.bt.trackerListUrl = nextSettings.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL;
      form.bt.pauseUploadWhenLimitReached = nextSettings.bt.pauseUploadWhenLimitReached;
      form.bt.uploadLimitBytes = nextSettings.bt.uploadLimitBytes;
      form.bt.uploadRatioLimit = nextSettings.bt.uploadRatioLimit;
      form.bt.listenPort = nextSettings.bt.listenPort ?? null;
      form.bt.listenPortRange = nextSettings.bt.listenPortRange ?? null;
      form.bt.upnpEnabled = nextSettings.bt.upnpEnabled ?? false;
      form.bt.enableNatpmp = nextSettings.bt.enableNatpmp ?? true;
      form.bt.enableIpv6 = nextSettings.bt.enableIpv6 ?? true;
      form.bt.enablePex = nextSettings.bt.enablePex ?? true;
      form.bt.enableLsd = nextSettings.bt.enableLsd ?? true;
      form.bt.enableUtp = nextSettings.bt.enableUtp ?? true;
      form.bt.enableFastExtension = nextSettings.bt.enableFastExtension ?? true;
      form.bt.enableHolepunch = nextSettings.bt.enableHolepunch ?? true;
      form.bt.enableWebSeed = nextSettings.bt.enableWebSeed ?? true;
      form.bt.enableSuperSeeding = nextSettings.bt.enableSuperSeeding ?? false;
      form.bt.preallocateMode = nextSettings.bt.preallocateMode ?? "none";
      form.bt.encryptionMode = nextSettings.bt.encryptionMode ?? "enabled";
      form.bt.maxDownloads = nextSettings.bt.maxDownloads ?? 3;
      form.bt.maxSeeds = nextSettings.bt.maxSeeds ?? 5;
      form.bt.maxTorrents = nextSettings.bt.maxTorrents ?? 100;
      form.bt.activeLimit = nextSettings.bt.activeLimit ?? 500;
      form.bt.globalDownloadRateLimit = nextSettings.bt.globalDownloadRateLimit ?? 0;
      form.bt.globalUploadRateLimit = nextSettings.bt.globalUploadRateLimit ?? 0;
      form.logging.enabled = nextSettings.logging?.enabled ?? true;
      form.logging.level = nextSettings.logging?.level ?? "info";
      form.logging.filePath = nextSettings.logging?.filePath ?? "";
      form.logging.retentionCount = nextSettings.logging?.retentionCount ?? null;
      form.logging.retentionDays = nextSettings.logging?.retentionDays ?? null;
      form.aria2Rpc.enabled = nextSettings.aria2Rpc?.enabled ?? true;
      form.aria2Rpc.port = nextSettings.aria2Rpc?.port ?? 6800;
      form.aria2Rpc.secret = nextSettings.aria2Rpc?.secret ?? null;
      form.cdnAcceleration = { ...nextSettings.cdnAcceleration };
      form.githubMirror = {
        enabled: nextSettings.githubMirror?.enabled ?? false,
        mirrors: nextSettings.githubMirror?.mirrors?.map((mirror) => ({ ...mirror })) ?? [],
      };
      form.notifications = {
        enabled: nextSettings.notifications?.enabled ?? false,
      };
      form.ioBaseline = {
        bufferLimitMb: nextSettings.ioBaseline?.bufferLimitMb ?? 1024,
        gameModeBufferMb: nextSettings.ioBaseline?.gameModeBufferMb ?? 128,
        gameMode: nextSettings.ioBaseline?.gameMode ?? false,
        diskTypeOverrides: { ...nextSettings.ioBaseline?.diskTypeOverrides },
        maxParallelHdd: nextSettings.ioBaseline?.maxParallelHdd ?? 4,
        gameModeMaxParallel: nextSettings.ioBaseline?.gameModeMaxParallel ?? 1,
        hddBufferEnabled: nextSettings.ioBaseline?.hddBufferEnabled ?? true,
        ssdWriteCombineMb: nextSettings.ioBaseline?.ssdWriteCombineMb ?? 0,
      };
      form.autostart = nextSettings.autostart ?? false;
      form.setupCompleted = nextSettings.setupCompleted ?? false;
      form.lastSetupStep = nextSettings.lastSetupStep ?? null;
      form.maxInMemoryDownloads = nextSettings.maxInMemoryDownloads ?? 200;
      if (!form.doubleClick) {
        form.doubleClick = { onCompleted: "none", onUncompleted: "none" };
      }
      form.doubleClick.onCompleted = nextSettings.doubleClick?.onCompleted ?? "none";
      form.doubleClick.onUncompleted = nextSettings.doubleClick?.onUncompleted ?? "none";
      form.speedLimitSchedule = nextSettings.speedLimitSchedule?.slice() ?? [];
      savedSettingsSnapshot.value = serializeSettings(buildSettingsPayload());
      onDirtyChange?.(false);
    },
    { immediate: true },
  );

  // ── Dirty tracking ────────────────────────────────────────────────

  watch(
    settingsDraftSnapshotComputed,
    (snapshot) => {
      if (!savedSettingsSnapshot.value) {
        return;
      }

      onDirtyChange?.(snapshot !== savedSettingsSnapshot.value);
    },
    { immediate: true },
  );

  return { form, buildSettingsPayload, savedSettingsSnapshot };
}
