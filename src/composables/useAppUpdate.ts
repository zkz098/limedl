import { ref, computed, readonly } from "vue";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useI18n } from "../i18n";
import { useNotification } from "./useNotification";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "newer"
  | "available"
  | "downloading"
  | "installing"
  | "done"
  | "error";

export type UpdateChannel = "stable" | "beta";

const CHANNEL_STORAGE_KEY = "flareget.updateChannel";
const STARTUP_CHECK_TIMEOUT_MS = 5_000;

// ── Module-level singleton state ──────────────────────────────────

const status = ref<UpdateStatus>("idle");
const progressPercent = ref(0);
const totalBytes = ref(0);
const downloadedBytes = ref(0);
const currentVersion = ref("");
const latestVersion = ref("");
const latestBody = ref("");
const latestDate = ref("");
const errorMessage = ref("");
const updateAvailable = ref(false);

const channel = ref<UpdateChannel>(
  (() => {
    const stored = localStorage.getItem(CHANNEL_STORAGE_KEY);
    if (stored === "stable" || stored === "beta") {
      return stored;
    }
    return "stable";
  })(),
);

// Holds the Update object from the last check so downloadAndInstall
// can reuse it without an extra network request.
let pendingUpdate: Update | null = null;

// ── Computed ──────────────────────────────────────────────────────

const isChecking = computed(() => status.value === "checking");
const isDownloading = computed(() => status.value === "downloading");
const isInstalling = computed(() => status.value === "installing");

// ── Helpers ───────────────────────────────────────────────────────

function normalizeVersion(ver: string): string {
  return ver.startsWith("v") ? ver.slice(1) : ver;
}

function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const va = pa[i] ?? 0;
    const vb = pb[i] ?? 0;
    if (va > vb) return 1;
    if (va < vb) return -1;
  }
  return 0;
}

function isBusy(): boolean {
  return (
    status.value === "checking" || status.value === "downloading" || status.value === "installing"
  );
}

// ── Actions ───────────────────────────────────────────────────────

function setChannel(ch: UpdateChannel) {
  channel.value = ch;
  localStorage.setItem(CHANNEL_STORAGE_KEY, ch);
  // Reset state so user can re-check on the new channel
  status.value = "idle";
  errorMessage.value = "";
  pendingUpdate = null;
}

async function checkForUpdates(silent = false) {
  const { t } = useI18n();
  const { notifyInfo, notifyError } = useNotification();

  if (isBusy()) return null;

  status.value = "checking";
  errorMessage.value = "";
  pendingUpdate = null;

  try {
    const update = await check({
      timeout: STARTUP_CHECK_TIMEOUT_MS,
    });

    currentVersion.value = normalizeVersion(update?.currentVersion ?? "0.0.0");

    if (!update) {
      status.value = "up-to-date";
      updateAvailable.value = false;
      return null;
    }

    const latest = normalizeVersion(update.version);
    latestVersion.value = latest;

    // Prevent downgrade
    if (compareVersions(currentVersion.value, latest) >= 0) {
      status.value = silent ? "up-to-date" : "newer";
      updateAvailable.value = false;
      return null;
    }

    latestBody.value = update.body ?? "";
    latestDate.value = update.date ?? "";

    status.value = "available";
    updateAvailable.value = true;
    pendingUpdate = update;

    if (!silent) {
      notifyInfo(t("settings.aboutUpdateAvailable") + `: v${latest}`);
    }

    return update;
  } catch (err) {
    status.value = "error";
    errorMessage.value = err instanceof Error ? err.message : String(err);
    if (!silent) {
      notifyError(t("settings.aboutCheckingFailed"));
    }
    return null;
  }
}

async function downloadAndInstall() {
  const { t } = useI18n();
  const { notifySuccess, notifyError } = useNotification();

  if (isBusy()) return;

  status.value = "downloading";
  progressPercent.value = 0;
  totalBytes.value = 0;
  downloadedBytes.value = 0;
  errorMessage.value = "";

  try {
    // Reuse the update from checkForUpdates if available, otherwise fetch fresh
    const update = pendingUpdate ?? (await check({ timeout: 30_000 }));
    if (!update) {
      status.value = "up-to-date";
      return;
    }

    await update.downloadAndInstall((event: DownloadEvent) => {
      switch (event.event) {
        case "Started":
          totalBytes.value = event.data.contentLength ?? 0;
          break;
        case "Progress":
          downloadedBytes.value += event.data.chunkLength;
          if (totalBytes.value > 0) {
            progressPercent.value = Math.min(
              Math.round((downloadedBytes.value / totalBytes.value) * 100),
              99,
            );
          }
          break;
        case "Finished":
          progressPercent.value = 100;
          break;
      }
    });

    status.value = "installing";
    pendingUpdate = null;

    notifySuccess(t("settings.aboutRelaunchHint"));
    await relaunch();
  } catch (err) {
    status.value = "error";
    const msg = err instanceof Error ? err.message : String(err);
    const lower = msg.toLowerCase();
    if (lower.includes("disk") || lower.includes("space")) {
      errorMessage.value = t("settings.aboutDiskSpaceInsufficient");
    } else if (lower.includes("signature") || lower.includes("verify")) {
      errorMessage.value = t("settings.aboutSignatureInvalid");
    } else {
      errorMessage.value = msg;
    }
    pendingUpdate = null;
    notifyError(t("settings.aboutDownloadFailed"));
  }
}

/**
 * Silent startup check: runs after app mounts, doesn't show errors on failure.
 * Sets `updateAvailable` if a new version is found (for red dot badge).
 */
async function runStartupCheck() {
  if (status.value !== "idle") return;
  await checkForUpdates(true);
}

/**
 * Mark the update notification as acknowledged (clear red dot).
 */
function acknowledgeUpdate() {
  updateAvailable.value = false;
}

// ── Public API ────────────────────────────────────────────────────

export function useAppUpdate() {
  return {
    // State (read-only)
    status: readonly(status),
    progressPercent: readonly(progressPercent),
    totalBytes: readonly(totalBytes),
    downloadedBytes: readonly(downloadedBytes),
    currentVersion: readonly(currentVersion),
    latestVersion: readonly(latestVersion),
    latestBody: readonly(latestBody),
    latestDate: readonly(latestDate),
    errorMessage: readonly(errorMessage),
    updateAvailable: readonly(updateAvailable),
    channel: readonly(channel),

    // Computed
    isChecking,
    isDownloading,
    isInstalling,

    // Actions
    setChannel,
    checkForUpdates,
    downloadAndInstall,
    runStartupCheck,
    acknowledgeUpdate,
  };
}
