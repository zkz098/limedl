<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useI18n } from "../../i18n";
import logoUrl from "../../assets/logo.webp";
import { useAppUpdate } from "../../composables/useAppUpdate";
import { useNotificationStore } from "../../stores/notification";
import { saveAppSettings, factoryReset } from "../../lib/tauri/settings-api";
import type { AppSettings } from "../../types/settings";
import { relaunch, exit } from "@tauri-apps/plugin-process";
import { platform, arch, version as osVersion } from "@tauri-apps/plugin-os";
import { getVersion, getName, getTauriVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import SettingsSection from "./SettingsSection.vue";
import SettingsField from "./SettingsField.vue";
import UiButton from "../ui/UiButton.vue";
import UiProgress from "../ui/UiProgress.vue";
import ConfirmDialog from "../ui/ConfirmDialog.vue";

const GITHUB_REPO_URL = "https://github.com/zkz098/limedl";

async function openGitHub() {
  try {
    await openUrl(GITHUB_REPO_URL);
  } catch (err) {
    console.error("Failed to open GitHub URL:", err);
  }
}

const { t } = useI18n();

const emit = defineEmits<{
  "restart-setup": [];
}>();
const {
  status,
  progressPercent,
  totalBytes,
  downloadedBytes,
  currentVersion,
  latestVersion,
  latestBody,
  latestDate,
  errorMessage,
  updateAvailable,
  channel,
  isChecking,
  isDownloading,
  isInstalling,
  setChannel,
  checkForUpdates,
  downloadAndInstall,
  acknowledgeUpdate,
} = useAppUpdate();

// System info
const appName = ref("");
const appVersion = ref("");
const tauriVer = ref("");
const osPlatform = ref("");
const osArch = ref("");
const osVer = ref("");

onMounted(async () => {
  try {
    appName.value = await getName();
    appVersion.value = await getVersion();
    tauriVer.value = await getTauriVersion();
  } catch (err) {
    console.error("Failed to get app info:", err);
  }
  try {
    osPlatform.value = platform();
    osArch.value = arch();
    osVer.value = osVersion();
  } catch (err) {
    console.error("Failed to get OS info:", err);
  }
});

// Reset to default settings
const { notifySuccess, notifyError } = useNotificationStore();
const isResetting = ref(false);
const showResetConfirm = ref(false);

function buildDefaultSettings() {
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
      mode: "automatic" as const,
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 16,
        maxThreadsPerTask: 8,
        minThreadsPerTask: 0,
        adaptiveProfile: "balanced" as const,
      },
      chunkSizeStrategy: "adaptive" as const,
      tailSprintEnabled: false,
      connectionWarmupEnabled: true,
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 5,
      defaultChecksum: "blake3" as const,
      defaultUserAgent: "",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "",
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
      preallocateMode: "none" as const,
      encryptionMode: "enabled" as const,
      maxDownloads: 3,
      maxSeeds: 5,
      maxTorrents: 100,
      activeLimit: 500,
    },
    logging: {
      enabled: true,
      level: "info" as const,
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: false, port: 6800, secret: null, corsAllowedOrigins: [] },
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
      maxParallelHdd: 4,
      gameModeMaxParallel: 1,
      hddBufferEnabled: true,
      ssdWriteCombineMb: 0,
      diskTypeOverrides: {},
    },
    downloadLimits: null,
    autostart: false,
    setupCompleted: true,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    doubleClick: { onCompleted: "none", onUncompleted: "none" },
    speedLimitSchedule: [],
  } as AppSettings;
}

async function handleResetSettings() {
  if (isResetting.value) return;
  isResetting.value = true;
  try {
    const defaults = buildDefaultSettings();
    await saveAppSettings(defaults);
    notifySuccess(t("settings.aboutResetSuccess"));
    emit("restart-setup");
  } catch (err) {
    console.error("Failed to reset settings:", err);
    notifyError(t("settings.aboutResetFailed"));
  } finally {
    isResetting.value = false;
  }
}

function handleResetClick() {
  showResetConfirm.value = true;
}

function confirmReset() {
  showResetConfirm.value = false;
  handleResetSettings();
}

function cancelReset() {
  showResetConfirm.value = false;
}

const showFactoryResetConfirm = ref(false);
const isFactoryResetting = ref(false);

async function handleFactoryReset() {
  if (isFactoryResetting.value) return;
  isFactoryResetting.value = true;
  try {
    await factoryReset();
    notifySuccess(t("settings.aboutFactoryResetSuccess"));
    await relaunch();
    await exit(0);
  } catch (err) {
    console.error("Factory reset failed:", err);
    notifyError(t("settings.aboutFactoryResetFailed"));
  } finally {
    isFactoryResetting.value = false;
    showFactoryResetConfirm.value = false;
  }
}

const channelOptions = computed(() => [
  { label: t("settings.aboutChannelStable"), value: "stable" as const },
  { label: t("settings.aboutChannelBeta"), value: "beta" as const },
]);

const statusLabel = computed(() => {
  switch (status.value) {
    case "checking":
      return t("settings.aboutChecking");
    case "up-to-date":
      return t("settings.aboutUpToDate");
    case "newer":
      return t("settings.aboutChannelDowngradeWarning");
    case "available":
      return t("settings.aboutUpdateAvailable");
    case "downloading":
      return t("settings.aboutDownloading");
    case "installing":
      return t("settings.aboutInstalling");
    case "error":
      return errorMessage.value || t("settings.aboutCheckingFailed");
    default:
      return "";
  }
});

const formattedDownloaded = computed(() => {
  if (downloadedBytes.value === 0) return "";
  if (downloadedBytes.value < 1024 * 1024) {
    return `${(downloadedBytes.value / 1024).toFixed(1)} KB`;
  }
  return `${(downloadedBytes.value / (1024 * 1024)).toFixed(1)} MB`;
});

const formattedTotal = computed(() => {
  if (totalBytes.value === 0) return "";
  return `${(totalBytes.value / (1024 * 1024)).toFixed(1)} MB`;
});

const progressLabel = computed(() => {
  const size = formattedTotal.value
    ? `${formattedDownloaded.value} / ${formattedTotal.value}`
    : formattedDownloaded.value;
  return size ? `${size} · ${progressPercent.value}%` : `${progressPercent.value}%`;
});

async function handleCheck() {
  await checkForUpdates(false);
  // Only clear the red dot if no update was found (updateAvailable is a readonly ref, accessed via .value)
  if (!updateAvailable) {
    acknowledgeUpdate();
  }
}

function handleChannelChange(value: "stable" | "beta") {
  setChannel(value);
}

const changelogLines = computed(() => {
  const body = latestBody.value;
  if (!body) return [];
  return body.split("\n").filter((l) => l.trim().length > 0);
});

const showVersionBadge = computed(() => {
  const s = status.value;
  return s !== "idle" && s !== "checking";
});

const versionBadgeClass = computed(() => {
  switch (status.value) {
    case "available":
      return "about-version-badge--update";
    case "error":
      return "about-version-badge--error";
    case "newer":
      return "about-version-badge--warning";
    default:
      return "about-version-badge--current";
  }
});
</script>

<template>
  <div class="about-panel-wrapper">
  <div class="about-panel flex flex-col gap-5">
    <!-- Card 1: About Limedl -->
    <SettingsSection :title="t('settings.aboutTitle')" icon="i-ri-information-line">
      <div class="about-identity">
        <img :src="logoUrl" alt="Limedl" class="about-identity__logo" />
        <div class="about-identity__meta">
          <span class="about-identity__name">{{ appName || "Limedl" }}</span>
          <span class="about-identity__version">v{{ appVersion || "--" }}</span>
          <div class="about-identity__system about-system">
            <span class="about-system__item">
              <span class="about-system__label">{{ t("settings.aboutOs") }}</span>
              <span class="about-system__value">{{ osPlatform || "\u2014" }}</span>
            </span>
            <span class="about-system__sep" aria-hidden="true">·</span>
            <span class="about-system__item">
              <span class="about-system__label">{{ t("settings.aboutArchitecture") }}</span>
              <span class="about-system__value">{{ osArch || "\u2014" }}</span>
            </span>
            <span class="about-system__sep" aria-hidden="true">·</span>
            <span class="about-system__item">
              <span class="about-system__label">{{ t("settings.aboutOsVersion") }}</span>
              <span class="about-system__value">{{ osVer || "\u2014" }}</span>
            </span>
            <span class="about-system__sep" aria-hidden="true">·</span>
            <span class="about-system__item">
              <span class="about-system__label">{{ t("settings.aboutTauriVersion") }}</span>
              <span class="about-system__value">{{ tauriVer || "\u2014" }}</span>
            </span>
          </div>
        </div>
      </div>

      <div class="about-license">
        <span class="i-ri-scales-line about-license__icon" aria-hidden="true" />
        <div class="about-license__content">
          <p class="about-license__text">
            {{ t("settings.aboutLicense") }}
          </p>
          <p class="about-license__ref">{{ t("settings.aboutLicenseRef") }}</p>
        </div>
      </div>
    </SettingsSection>

    <!-- Card 2: Software Update -->
    <SettingsSection :title="t('settings.aboutUpdateTitle')" icon="i-ri-download-cloud-line">
      <div class="update-card">
        <div class="update-card__header">
          <div class="about-version-badge" :class="versionBadgeClass">
            <span class="about-version-badge__text">{{
              showVersionBadge ? currentVersion || appVersion : "?"
            }}</span>
          </div>
          <div class="update-card__status">
            <span class="update-card__label">{{ t("settings.aboutVersion") }}</span>
            <span class="update-card__state">{{ statusLabel }}</span>
          </div>

          <div v-if="latestVersion && status === 'available'" class="update-card__target">
            <span class="i-ri-arrow-right-line update-card__target-icon" aria-hidden="true" />
            <span class="update-card__target-version">v{{ latestVersion }}</span>
          </div>
        </div>

        <div class="settings-grid">
          <SettingsField
            :wide="true"
            :label="t('settings.aboutChannel')"
            :hint="channel === 'beta' ? t('settings.aboutChannelDowngradeWarning') : undefined"
          >
            <div class="update-card__channel">
              <UiButton
                v-for="opt in channelOptions"
                :key="opt.value"
                size="sm"
                :variant="channel === opt.value ? 'primary' : 'secondary'"
                @click="handleChannelChange(opt.value)"
              >
                {{ opt.label }}
              </UiButton>
            </div>
          </SettingsField>
        </div>

        <div class="update-card__actions">
          <UiButton
            v-if="status !== 'downloading' && status !== 'installing' && status !== 'available'"
            icon="i-ri-refresh-line"
            :loading="isChecking"
            @click="handleCheck"
          >
            {{ isChecking ? t("settings.aboutChecking") : t("settings.aboutCheckUpdate") }}
          </UiButton>

          <UiButton
            v-if="status === 'available'"
            variant="primary"
            icon="i-ri-download-2-line"
            :loading="isDownloading || isInstalling"
            @click="downloadAndInstall"
          >
            {{ isDownloading ? t("settings.aboutDownloading") : t("settings.aboutUpdateNow") }}
          </UiButton>
        </div>

        <div
          v-if="status === 'downloading' || status === 'installing'"
          class="update-card__progress"
        >
          <UiProgress :value="progressPercent" :show-label="true" :label="progressLabel" />
          <span v-if="status === 'installing'" class="update-card__hint">
            {{ t("settings.aboutRelaunchHint") }}
          </span>
        </div>

        <div v-if="status === 'error' && errorMessage" class="status-banner status-banner--error">
          <span class="i-ri-error-warning-line" aria-hidden="true" />
          <span>{{ errorMessage }}</span>
        </div>
      </div>
    </SettingsSection>

    <!-- Card 3: Changelog -->
    <SettingsSection
      v-if="latestBody && changelogLines.length > 0"
      :title="t('settings.aboutChangelog')"
      icon="i-ri-file-list-line"
    >
      <div class="update-card__changelog">
        <span v-if="latestDate" class="update-card__changelog-date">
          {{ t("settings.aboutReleaseDate") }}: {{ latestDate }}
        </span>
        <div class="about-changelog">
          <p
            v-for="(line, i) in changelogLines"
            :key="i"
            :class="{ 'about-changelog__heading': line.startsWith('##') }"
          >
            {{ line }}
          </p>
        </div>
      </div>
    </SettingsSection>

    <!-- Card 4: Links & Actions -->
    <SettingsSection :title="t('settings.aboutLinksTitle')" icon="i-ri-links-line">
      <div class="about-links">
        <UiButton
          v-if="!showResetConfirm"
          variant="secondary"
          icon="i-ri-refresh-line"
          @click="handleResetClick"
        >
          {{ t("settings.aboutResetSettings") }}
        </UiButton>

        <div v-if="showResetConfirm" class="about-reset-confirm">
          <span class="about-reset-confirm__text">{{ t("settings.aboutResetConfirm") }}</span>
          <div class="about-reset-confirm__actions">
            <UiButton variant="danger" size="sm" :loading="isResetting" @click="confirmReset">
              {{ t("common.confirm") }}
            </UiButton>
            <UiButton variant="ghost" size="sm" :disabled="isResetting" @click="cancelReset">
              {{ t("common.cancel") }}
            </UiButton>
          </div>
        </div>

        <UiButton variant="secondary" icon="i-ri-restart-line" @click="emit('restart-setup')">
          {{ t("settings.aboutRestartSetupButton") }}
        </UiButton>

        <UiButton
          variant="danger"
          icon="i-ri-delete-bin-line"
          @click="showFactoryResetConfirm = true"
        >
          {{ t("settings.aboutFactoryResetButton") }}
        </UiButton>

        <UiButton
          variant="secondary"
          icon="i-ri-github-fill"
          icon-right="i-ri-external-link-line"
          @click="openGitHub"
        >
          {{ t("settings.aboutGitHubLink") }}
        </UiButton>
      </div>
    </SettingsSection>
  </div>

  <ConfirmDialog
    :model-value="showFactoryResetConfirm"
    :kicker="t('settings.aboutFactoryResetTitle')"
    :title="t('settings.aboutFactoryResetTitle')"
    :message="t('settings.aboutFactoryResetMessage')"
    :confirm-text="t('settings.aboutFactoryResetConfirm')"
    :cancel-text="t('common.cancel')"
    icon="i-ri-delete-bin-line"
    :icon-danger="true"
    confirm-icon="i-ri-delete-bin-line"
    :confirm-loading="isFactoryResetting"
    @cancel="showFactoryResetConfirm = false"
    @confirm="handleFactoryReset"
  />
  </div>
</template>

<style scoped>
.about-panel {
  max-width: 48rem;
}

.about-identity {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin-bottom: var(--space-5);
}

.about-identity__logo {
  width: 4rem;
  height: 4rem;
  flex-shrink: 0;
  border-radius: var(--radius-lg);
  background: var(--color-surface-muted);
  object-fit: contain;
}

.about-identity__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.about-identity__name {
  font-size: var(--font-size-hero);
  font-weight: var(--font-weight-display);
  color: var(--color-heading);
  line-height: var(--line-height-display);
  letter-spacing: var(--letter-spacing-tight);
}

.about-identity__version {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.about-identity__system {
  margin-top: var(--space-2);
}

.about-system {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1) var(--space-2);
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.about-system__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.about-system__label {
  color: var(--color-text-soft);
}

.about-system__value {
  color: var(--color-text-main);
  font-weight: var(--font-weight-semibold);
}

.about-system__sep {
  color: var(--color-text-soft);
  user-select: none;
}

.about-license {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4);
  border: var(--border-width-thin) solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
}

.about-license__icon {
  flex-shrink: 0;
  margin-top: var(--space-1);
  font-size: var(--font-size-metric);
  color: var(--color-text-muted);
}

.about-license__content {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.about-license__text {
  margin: 0;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  color: var(--color-text-muted);
}

.about-license__ref {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--color-text-soft);
}

.update-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.update-card__header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.about-version-badge {
  width: 3.5rem;
  height: 3.5rem;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-lg);
  flex-shrink: 0;
}

.about-version-badge__text {
  font-size: var(--font-size-metric);
  font-weight: var(--font-weight-display);
}

.about-version-badge--update {
  background: var(--color-accent-soft);
  color: var(--color-accent-strong);
}

.about-version-badge--current {
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
}

.about-version-badge--error {
  background: var(--color-danger-bg);
  color: var(--color-danger-text);
}

.about-version-badge--warning {
  background: var(--color-warning-bg);
  color: var(--color-warning-text);
}

.update-card__status {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.update-card__label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.update-card__state {
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
}

.update-card__target {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-left: auto;
  color: var(--color-accent-strong);
}

.update-card__target-icon {
  font-size: var(--font-size-body);
}

.update-card__target-version {
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
}

.update-card__channel {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.update-card__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.update-card__progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.update-card__hint {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.update-card__changelog {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.update-card__changelog-date {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.about-changelog {
  white-space: pre-wrap;
  overflow-wrap: break-word;
  font-size: var(--font-size-small);
  line-height: var(--line-height-tight);
  color: var(--color-text-main);
}

.about-changelog p {
  margin: 0;
}

.about-changelog__heading {
  font-weight: var(--font-weight-semibold);
  color: var(--color-heading);
  margin-top: var(--space-2);
}

.about-links {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.about-reset-confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--color-warning-bg);
  border: 1px solid var(--color-warning-border);
}

.about-reset-confirm__text {
  font-size: var(--font-size-small);
  color: var(--color-warning-text);
  flex: 1;
  min-width: 0;
}

.about-reset-confirm__actions {
  display: flex;
  gap: var(--space-2);
  flex-shrink: 0;
}

@media (max-width: 680px) {
  .about-identity {
    flex-direction: column;
    align-items: flex-start;
  }

  .about-identity__meta {
    width: 100%;
  }

  .update-card__target {
    margin-left: 0;
    width: 100%;
  }

  .about-links {
    flex-direction: column;
  }

  .about-links :deep(.ui-button) {
    width: 100%;
  }
}
</style>
