<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "../../i18n";
import { useAppUpdate } from "../../composables/useAppUpdate";
import SettingsSection from "./SettingsSection.vue";
import SettingsField from "./SettingsField.vue";
import UiButton from "../ui/UiButton.vue";

const { t } = useI18n();
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
</script>

<template>
  <div class="about-panel flex flex-col gap-5">
    <!-- Version Info -->
    <SettingsSection :title="t('settings.aboutTitle')" icon="i-ri-information-line">
      <div class="flex flex-col gap-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div
              class="about-version-badge w-14 h-14 flex items-center justify-center rounded-xl"
              :class="{
                'bg-[var(--color-accent-soft)] text-[var(--color-accent-strong)]': updateAvailable && status === 'available',
                'bg-[var(--color-surface-muted)] text-[var(--color-text-muted)]': status === 'up-to-date' || status === 'idle',
                'bg-[var(--color-error-soft)] text-[var(--color-error)]': status === 'error',
                'bg-[var(--color-warning-soft)] text-[var(--color-warning)]': status === 'newer',
              }"
            >
              <span class="text-xl font-bold">{{ showVersionBadge ? (currentVersion || "0.1.0") : "?" }}</span>
            </div>
            <div class="flex flex-col gap-0.5">
              <span class="text-sm text-[var(--color-text-muted)]">
                {{ t("settings.aboutVersion") }}
              </span>
              <span class="text-sm font-semibold text-[var(--color-heading)]">
                {{ statusLabel }}
              </span>
            </div>
          </div>

          <div v-if="latestVersion && status === 'available'" class="flex items-center gap-1.5">
            <span class="i-ri-arrow-right-line text-[var(--color-accent-strong)]" aria-hidden="true" />
            <span class="text-sm font-semibold text-[var(--color-accent-strong)]">v{{ latestVersion }}</span>
          </div>
        </div>
      </div>
    </SettingsSection>

    <!-- Channel Selector -->
    <SettingsSection :title="t('settings.aboutChannel')">
      <div class="settings-grid">
        <SettingsField
          :wide="true"
          :label="t('settings.aboutChannel')"
          :hint="channel === 'beta' ? t('settings.aboutChannelDowngradeWarning') : undefined"
        >
          <div class="flex gap-2">
            <button
              v-for="opt in channelOptions"
              :key="opt.value"
              type="button"
              class="about-channel-btn px-4 py-2 rounded-md border text-sm font-medium cursor-pointer transition-colors duration-150"
              :class="channel === opt.value
                ? 'border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] text-[var(--color-accent-strong)]'
                : 'border-[var(--color-border)] bg-[var(--color-input-bg)] text-[var(--color-text-main)] hover:border-[var(--color-border-strong)]'"
              @click="handleChannelChange(opt.value)"
            >
              {{ opt.label }}
            </button>
          </div>
        </SettingsField>
      </div>
    </SettingsSection>

    <!-- Download Progress -->
    <SettingsSection
      v-if="status === 'downloading' || status === 'installing'"
      :title="status === 'installing' ? t('settings.aboutInstalling') : t('settings.aboutDownloading')"
    >
      <div class="flex flex-col gap-3">
        <div class="flex items-center justify-between text-xs text-[var(--color-text-muted)]">
          <span>{{ formattedDownloaded }}{{ formattedTotal ? ` / ${formattedTotal}` : "" }}</span>
          <span>{{ progressPercent }}%</span>
        </div>
        <div class="about-progress-track w-full h-2 rounded-full bg-[var(--color-surface-muted)] overflow-hidden">
          <div
            class="about-progress-fill h-full rounded-full transition-[width] duration-300 ease-out bg-[var(--color-accent-strong)]"
            :style="{ width: `${progressPercent}%` }"
          />
        </div>
        <span v-if="status === 'installing'" class="text-xs text-[var(--color-text-muted)]">
          {{ t("settings.aboutRelaunchHint") }}
        </span>
      </div>
    </SettingsSection>

    <!-- Changelog -->
    <SettingsSection
      v-if="latestBody && changelogLines.length > 0"
      :title="t('settings.aboutChangelog')"
    >
      <div class="flex flex-col gap-1">
        <span
          v-if="latestDate"
          class="text-xs text-[var(--color-text-muted)]"
        >
          {{ t("settings.aboutReleaseDate") }}: {{ latestDate }}
        </span>
        <div class="about-changelog text-sm leading-relaxed text-[var(--color-text-main)] mt-2">
          <p
            v-for="(line, i) in changelogLines"
            :key="i"
            class="m-0"
            :class="{ 'font-semibold text-[var(--color-heading)]': line.startsWith('##') }"
          >
            {{ line }}
          </p>
        </div>
      </div>
    </SettingsSection>

    <!-- Error display -->
    <SettingsSection
      v-if="status === 'error' && errorMessage"
      title=""
    >
      <div class="flex items-start gap-2 p-3 rounded-md bg-[var(--color-error-soft)] text-[var(--color-error)]">
        <span class="i-ri-error-warning-line flex-shrink-0 mt-0.5" aria-hidden="true" />
        <span class="text-sm">{{ errorMessage }}</span>
      </div>
    </SettingsSection>

    <!-- Action buttons -->
    <div class="flex items-center gap-3">
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
  </div>
</template>

<style scoped>
.about-panel {
  max-width: 48rem;
}

.about-channel-btn:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--color-focus-ring);
}

.about-changelog {
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
