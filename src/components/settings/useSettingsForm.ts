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
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
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
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 5,
      defaultChecksum: "blake3",
      defaultUserAgent: DEFAULT_HTTP_USER_AGENT,
    },
    bt: {
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: DEFAULT_TRACKER_LIST_URL,
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      defaultDownloadSpeedLimit: 0,
      defaultUploadSpeedLimit: 0,
    },
    logging: {
      enabled: true,
      level: "info",
      filePath: "",
    },
    aria2Rpc: {
      enabled: true,
      port: 6800,
      secret: null,
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
    },
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
        sortKey: form.appearance.sortKey,
        sortDirection: form.appearance.sortDirection,
        compactView: form.appearance.compactView,
        visibleColumns: form.appearance.visibleColumns,
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
      },
      download: {
        defaultDownloadDir: form.download.defaultDownloadDir,
        defaultMaxRetries: form.download.defaultMaxRetries,
        defaultChecksum: form.download.defaultChecksum,
        defaultUserAgent: form.download.defaultUserAgent,
      },
      bt: {
        dhtEnabled: form.bt.dhtEnabled,
        trackerList: form.bt.trackerList,
        trackerListUrl: form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL,
        pauseUploadWhenLimitReached: form.bt.pauseUploadWhenLimitReached,
        uploadLimitBytes: form.bt.uploadLimitBytes,
        uploadRatioLimit: form.bt.uploadRatioLimit,
        // TODO: Add defaultDownloadSpeedLimit and defaultUploadSpeedLimit to save payload
        // once backend BtSettings in src-tauri/src/download/types.rs supports them.
      },
      logging: {
        enabled: form.logging.enabled,
        level: form.logging.level,
        filePath: form.logging.filePath.trim(),
      },
      aria2Rpc: {
        enabled: form.aria2Rpc.enabled,
        port: form.aria2Rpc.port,
        secret: form.aria2Rpc.secret?.trim() || null,
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
        gameMode: form.ioBaseline.gameMode ?? false,
        diskTypeOverrides: { ...form.ioBaseline.diskTypeOverrides },
      },
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
      form.bt.defaultDownloadSpeedLimit = nextSettings.bt.defaultDownloadSpeedLimit ?? 0;
      form.bt.defaultUploadSpeedLimit = nextSettings.bt.defaultUploadSpeedLimit ?? 0;
      form.logging.enabled = nextSettings.logging?.enabled ?? true;
      form.logging.level = nextSettings.logging?.level ?? "info";
      form.logging.filePath = nextSettings.logging?.filePath ?? "";
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
      };
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
