import { ref, computed, readonly } from "vue";
import { invoke } from "#invoke";
import { listen } from "#event";
import type { UnlistenFn } from "#event";
import { useI18n } from "../i18n";
import { useNotificationStore } from "../stores/notification";

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

interface CheckUpdateFullResult {
  version: string;
  body?: string;
  date?: string;
  downloadUrl: string;
  signature: string;
  currentVersion: string;
}

interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
}

const CHANNEL_STORAGE_KEY = "limedl.updateChannel";

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
  status.value = "idle";
  errorMessage.value = "";
}

async function checkForUpdates(silent = false) {
  const { t } = useI18n();
  const { notifyInfo, notifyError } = useNotificationStore();

  if (isBusy()) return null;

  status.value = "checking";
  errorMessage.value = "";

  try {
    const result = await invoke<CheckUpdateFullResult | null>("check_update_full");

    if (!result) {
      status.value = "up-to-date";
      updateAvailable.value = false;
      return null;
    }

    currentVersion.value = normalizeVersion(result.currentVersion);

    const latest = normalizeVersion(result.version);
    latestVersion.value = latest;

    if (compareVersions(currentVersion.value, latest) >= 0) {
      status.value = silent ? "up-to-date" : "newer";
      updateAvailable.value = false;
      return null;
    }

    latestBody.value = result.body ?? "";
    latestDate.value = result.date ?? "";

    status.value = "available";
    updateAvailable.value = true;

    if (!silent) {
      notifyInfo(t("settings.aboutUpdateAvailable") + `: v${latest}`);
    }

    return result;
  } catch (err) {
    status.value = "error";
    errorMessage.value = err instanceof Error ? err.message : String(err);
    if (!silent) {
      notifyError(t("settings.aboutCheckingFailed"));
    }
    return null;
  }
}

/**
 * Download the update via limedl's own download engine, verify the
 * signature, and launch the platform-native installer.
 *
 * The Rust command (`download_and_install_update`) is self-contained:
 * it checks for updates, downloads via limedl-core, verifies the
 * minisign signature, and triggers the installer — all in one call.
 *
 * On Windows the installer calls `process::exit(0)`, so the invoke
 * promise never resolves. The last signal to the frontend is the
 * `update-installing` Tauri event, which transitions the UI to the
 * "installing" state.
 */
async function downloadAndInstall() {
  const { t } = useI18n();
  const { notifyError } = useNotificationStore();

  if (isBusy()) return;

  status.value = "downloading";
  progressPercent.value = 0;
  totalBytes.value = 0;
  downloadedBytes.value = 0;
  errorMessage.value = "";

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenInstalling: UnlistenFn | null = null;

  try {
    // Set up progress listener before calling the command
    unlistenProgress = await listen<UpdateDownloadProgress>(
      "update-download-progress",
      (event) => {
        totalBytes.value = event.payload.totalBytes;
        downloadedBytes.value = event.payload.downloadedBytes;
        progressPercent.value = event.payload.percent;
      },
    );

    // Set up installing listener (fire-and-forget signal before the
    // installer process takes over)
    unlistenInstalling = await listen("update-installing", () => {
      status.value = "installing";
      updateAvailable.value = false;
    });

    // This call is self-contained: check → download → verify → install.
    // On Windows, the process exits during install and this never resolves.
    await invoke("download_and_install_update");
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
    notifyError(t("settings.aboutDownloadFailed"));
  } finally {
    if (unlistenProgress) unlistenProgress();
    if (unlistenInstalling) unlistenInstalling();
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
