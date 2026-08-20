import { computed, reactive, ref, watch, type Ref } from "vue";

import type { AppSettings } from "../../types/settings";
import { serializeSettings, settingsDraftSnapshot } from "./settingsUtils";
import { DEFAULT_HTTP_USER_AGENT, DEFAULT_TRACKER_LIST_URL } from "./useSettingsSummaries";
import { DEFAULT_APP_SETTINGS } from "../../lib/app-settings-defaults";

interface UseSettingsFormOptions {
  settings: Ref<AppSettings | null>;
  onDirtyChange?: (isDirty: boolean) => void;
}

/**
 * Shared settings form composable — eliminates ~180 lines of duplicated form
 * initialization, settings-sync watcher, dirty-tracking watcher, and payload
 * builder between SettingsPage.vue and LabsPage.vue.
 *
 * Sync policy: rather than copying fields one-by-one (which let a newly-added
 * setting silently drift out of the form — the root cause of the "connection
 * warmup resets after save" bug), the whole settings tree is shallow-merged
 * into the form at the sub-object granularity. Both `fillFormFromSettings`
 * (saved → form) and `buildSettingsPayload` (form → saved) are shape-driven,
 * so adding a field to a settings struct automatically flows through both
 * directions and cannot be forgotten.
 */
export function useSettingsForm(options: UseSettingsFormOptions) {
  const { settings, onDirtyChange } = options;

  // ── Reactive form ─────────────────────────────────────────────────

  const form = reactive<AppSettings>({
    ...DEFAULT_APP_SETTINGS,
    scheduler: { ...DEFAULT_APP_SETTINGS.scheduler, mode: "automatic" },
  });

  const savedSettingsSnapshot = ref("");

  // ── Sync: copy saved settings into the form (shape-driven merge) ──

  function fillFormFromSettings(next: AppSettings) {
    const base = DEFAULT_APP_SETTINGS;

    form.globalSpeedLimitBps = next.globalSpeedLimitBps ?? base.globalSpeedLimitBps ?? 0;
    form.appearance = { ...base.appearance, ...next.appearance };
    form.proxy = { ...base.proxy, ...next.proxy };
    form.scheduler = { ...base.scheduler, ...next.scheduler };
    form.download = { ...base.download, ...next.download };
    form.bt = { ...base.bt, ...next.bt };
    form.logging = { ...base.logging, ...next.logging };
    form.aria2Rpc = { ...base.aria2Rpc, ...next.aria2Rpc };
    form.cdnAcceleration = { ...base.cdnAcceleration, ...next.cdnAcceleration };
    form.githubMirror = { ...base.githubMirror, ...next.githubMirror };
    form.notifications = { ...base.notifications, ...next.notifications };
    form.ioBaseline = { ...base.ioBaseline, ...next.ioBaseline };
    form.doubleClick = { ...base.doubleClick, ...next.doubleClick };
    form.pet = { ...base.pet, ...next.pet };

    form.autostart = next.autostart ?? base.autostart ?? false;
    form.setupCompleted = next.setupCompleted ?? base.setupCompleted ?? false;
    form.lastSetupStep = next.lastSetupStep ?? base.lastSetupStep ?? null;
    form.maxInMemoryDownloads = next.maxInMemoryDownloads ?? base.maxInMemoryDownloads ?? 200;
    form.speedLimitSchedule = [...(next.speedLimitSchedule ?? base.speedLimitSchedule ?? [])];

    // Keep the explicit fallbacks the fields historically relied on.
    form.download.defaultUserAgent = form.download.defaultUserAgent || DEFAULT_HTTP_USER_AGENT;
    form.bt.trackerListUrl = form.bt.trackerListUrl || DEFAULT_TRACKER_LIST_URL;
  }

  // ── Payload: clone the form back into a saved-settings object ─────

  function buildSettingsPayload(): AppSettings {
    return {
      globalSpeedLimitBps: form.globalSpeedLimitBps ?? 0,
      pet: { ...form.pet },
      appearance: { ...form.appearance },
      proxy: { ...form.proxy },
      scheduler: { ...form.scheduler },
      download: { ...form.download },
      bt: { ...form.bt },
      logging: { ...form.logging, filePath: (form.logging.filePath ?? "").trim() },
      aria2Rpc: {
        ...form.aria2Rpc,
        secret: form.aria2Rpc.secret?.trim() || null,
        corsAllowedOrigins: form.aria2Rpc.corsAllowedOrigins ?? [],
      },
      cdnAcceleration: { ...form.cdnAcceleration },
      githubMirror: {
        enabled: form.githubMirror?.enabled ?? false,
        mirrors: form.githubMirror?.mirrors?.map((mirror) => ({ ...mirror })) ?? [],
      },
      notifications: { ...form.notifications },
      ioBaseline: {
        ...form.ioBaseline,
        bufferLimitMb: Math.max(64, Math.min(32768, form.ioBaseline.bufferLimitMb ?? 1024)),
        gameModeBufferMb: Math.max(16, Math.min(4096, form.ioBaseline.gameModeBufferMb ?? 128)),
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
      speedLimitSchedule: [...(form.speedLimitSchedule ?? [])],
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

      fillFormFromSettings(nextSettings);

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
