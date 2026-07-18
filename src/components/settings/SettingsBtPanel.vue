<script setup lang="ts">
import { computed, ref } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiTextField from "../ui/UiTextField.vue";
import UiSelect from "../ui/UiSelect.vue";
import UiSwitch from "../ui/UiSwitch.vue";
import type { AppSettings } from "../../types/settings";
import SettingsField from "./SettingsField.vue";
import SettingsSection from "./SettingsSection.vue";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
  btSummary: string;
  btUploadLimitMiB: number;
  isFetchingTrackerList: boolean;
  defaultTrackerListUrl: string;
}>();

const emit = defineEmits<{
  "update:btUploadLimitMiB": [value: number | null];
  fetchTrackerList: [];
}>();

const trackerListEntries = computed(() =>
  props.draft.bt.trackerList
    .split(/\r?\n/)
    .map((tracker) => tracker.trim())
    .filter(Boolean),
);

const pauseEnabled = computed(() => props.draft.bt.pauseUploadWhenLimitReached);

const trackerOpen = ref(true);
const seedingOpen = ref(true);
const networkOpen = ref(true);
const protocolsOpen = ref(true);
const diskSecurityOpen = ref(true);
const queueOpen = ref(true);
const rateLimitOpen = ref(true);

function toggleTracker() {
  trackerOpen.value = !trackerOpen.value;
}

function toggleSeeding() {
  seedingOpen.value = !seedingOpen.value;
}

function toggleNetwork() {
  networkOpen.value = !networkOpen.value;
}

function toggleProtocols() {
  protocolsOpen.value = !protocolsOpen.value;
}

function toggleDiskSecurity() {
  diskSecurityOpen.value = !diskSecurityOpen.value;
}

function toggleQueue() {
  queueOpen.value = !queueOpen.value;
}

function toggleRateLimit() {
  rateLimitOpen.value = !rateLimitOpen.value;
}
</script>

<template>
  <SettingsSection :title="t('settings.btTitle')" icon="i-ri-seedling-line" :summary="btSummary">
    <div class="bt-subgroups">
      <!-- Tracker -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': trackerOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="trackerOpen"
          @click="toggleTracker"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': trackerOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupTracker") }}</span>
        </button>
        <div v-show="trackerOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              wide
              :label="t('settings.btTrackerListUrl')"
              :info-tooltip="t('settings.btTrackerListUrlHint')"
            >
              <div class="settings-inline-field">
                <UiTextField
                  v-model="draft.bt.trackerListUrl"
                  type="url"
                  inputmode="url"
                  :placeholder="defaultTrackerListUrl"
                />
                <UiButton
                  type="button"
                  variant="secondary"
                  size="sm"
                  icon="i-ri-refresh-line"
                  :loading="isFetchingTrackerList"
                  @click="emit('fetchTrackerList')"
                >
                  {{
                    isFetchingTrackerList
                      ? t("settings.btTrackerListUpdating")
                      : t("settings.btTrackerListUpdate")
                  }}
                </UiButton>
              </div>
            </SettingsField>

            <SettingsField
              wide
              :label="t('settings.btTrackerList')"
              :info-tooltip="t('settings.btTrackerListHint', { count: trackerListEntries.length })"
            >
              <textarea
                v-model="draft.bt.trackerList"
                class="settings-textarea"
                :placeholder="t('settings.btTrackerListPlaceholder')"
                rows="5"
                spellcheck="false"
              />
            </SettingsField>
          </div>
        </div>
      </div>

      <!-- 做种与限制 -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': seedingOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="seedingOpen"
          @click="toggleSeeding"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': seedingOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupSeeding") }}</span>
        </button>
        <div v-show="seedingOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              wide
              :label="t('settings.btPauseUpload')"
              :info-tooltip="t('settings.btPauseUploadHint')"
            >
              <UiSwitch
                v-model="draft.bt.pauseUploadWhenLimitReached"
                :label="t('settings.btAutoPauseUpload')"
              />
            </SettingsField>

            <SettingsField
              :label="t('settings.btUploadLimit')"
              :hint="t('settings.btUploadLimitHint')"
            >
              <UiTextField
                type="number"
                :model-value="btUploadLimitMiB"
                :min="0"
                :max="10485760"
                :disabled="!pauseEnabled"
                unit="MiB"
                @update:model-value="emit('update:btUploadLimitMiB', $event as number | null)"
              />
            </SettingsField>

            <SettingsField
              :label="t('settings.btRatioLimit')"
              :hint="t('settings.btRatioLimitHint')"
            >
              <UiTextField
                type="number"
                :model-value="draft.bt.uploadRatioLimit"
                :min="0"
                :max="100"
                :step="0.1"
                :disabled="!pauseEnabled"
                unit="x"
                @update:model-value="draft.bt.uploadRatioLimit = Number($event ?? 0)"
              />
            </SettingsField>
          </div>
        </div>
      </div>

      <!-- 网络与端口 -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': networkOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="networkOpen"
          @click="toggleNetwork"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': networkOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupNetwork") }}</span>
        </button>
        <div v-show="networkOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              :label="t('settings.btListenPort')"
              :hint="t('settings.btListenPortHint')"
            >
              <UiTextField
                type="number"
                :model-value="draft.bt.listenPort"
                :min="1024"
                :max="65535"
                placeholder="42020"
                @update:model-value="draft.bt.listenPort = $event === null ? null : Number($event)"
              />
            </SettingsField>

            <SettingsField :label="t('settings.btUpnp')" :hint="t('settings.btUpnpHint')">
              <UiSwitch v-model="draft.bt.upnpEnabled" :label="t('settings.btUpnp')" />
            </SettingsField>

            <SettingsField :label="t('settings.btNatpmp')" :hint="t('settings.btNatpmpHint')">
              <UiSwitch v-model="draft.bt.enableNatpmp" :label="t('settings.btNatpmp')" />
            </SettingsField>

            <SettingsField :label="t('settings.btIpv6')" :hint="t('settings.btIpv6Hint')">
              <UiSwitch v-model="draft.bt.enableIpv6" :label="t('settings.btIpv6')" />
            </SettingsField>
          </div>
        </div>
      </div>

      <!-- 发现与协议 -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': protocolsOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="protocolsOpen"
          @click="toggleProtocols"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': protocolsOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupProtocols") }}</span>
        </button>
        <div v-show="protocolsOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField :label="t('settings.btDht')" :info-tooltip="t('settings.btDhtHint')">
              <UiSwitch v-model="draft.bt.dhtEnabled" :label="t('settings.btDhtNetwork')" />
            </SettingsField>

            <SettingsField :label="t('settings.btPex')" :hint="t('settings.btPexHint')">
              <UiSwitch v-model="draft.bt.enablePex" :label="t('settings.btPex')" />
            </SettingsField>

            <SettingsField :label="t('settings.btLsd')" :hint="t('settings.btLsdHint')">
              <UiSwitch v-model="draft.bt.enableLsd" :label="t('settings.btLsd')" />
            </SettingsField>

            <SettingsField :label="t('settings.btUtp')" :hint="t('settings.btUtpHint')">
              <UiSwitch v-model="draft.bt.enableUtp" :label="t('settings.btUtp')" />
            </SettingsField>

            <SettingsField
              :label="t('settings.btFastExtension')"
              :info-tooltip="t('settings.btFastExtensionHint')"
            >
              <UiSwitch
                v-model="draft.bt.enableFastExtension"
                :label="t('settings.btFastExtension')"
              />
            </SettingsField>

            <SettingsField
              :label="t('settings.btHolepunch')"
              :info-tooltip="t('settings.btHolepunchHint')"
            >
              <UiSwitch v-model="draft.bt.enableHolepunch" :label="t('settings.btHolepunch')" />
            </SettingsField>

            <SettingsField :label="t('settings.btWebSeed')" :hint="t('settings.btWebSeedHint')">
              <UiSwitch v-model="draft.bt.enableWebSeed" :label="t('settings.btWebSeed')" />
            </SettingsField>

            <SettingsField
              :label="t('settings.btSuperSeeding')"
              :info-tooltip="t('settings.btSuperSeedingHint')"
            >
              <UiSwitch
                v-model="draft.bt.enableSuperSeeding"
                :label="t('settings.btSuperSeeding')"
              />
            </SettingsField>
          </div>
        </div>
      </div>

      <!-- 磁盘与安全 -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': diskSecurityOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="diskSecurityOpen"
          @click="toggleDiskSecurity"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': diskSecurityOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupDiskSecurity") }}</span>
        </button>
        <div v-show="diskSecurityOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              :label="t('settings.btPreallocateMode')"
              :info-tooltip="t('settings.btPreallocateHint')"
            >
              <UiSelect
                v-model="draft.bt.preallocateMode"
                :options="[
                  { label: t('settings.btPreallocateNone'), value: 'none' },
                  { label: t('settings.btPreallocateFull'), value: 'full' },
                ]"
                :placeholder="t('settings.btPreallocateMode')"
              />
            </SettingsField>

            <SettingsField
              :label="t('settings.btEncryptionMode')"
              :info-tooltip="t('settings.btEncryptionHint')"
            >
              <UiSelect
                v-model="draft.bt.encryptionMode"
                :options="[
                  { label: t('settings.btEncryptionEnabled'), value: 'enabled' },
                  { label: t('settings.btEncryptionDisabled'), value: 'disabled' },
                  { label: t('settings.btEncryptionForced'), value: 'forced' },
                ]"
                :placeholder="t('settings.btEncryptionMode')"
              />
            </SettingsField>
          </div>
        </div>
      </div>

      <!-- 队列策略 -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': queueOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="queueOpen"
          @click="toggleQueue"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': queueOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupQueue") }}</span>
        </button>
        <div v-show="queueOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              :label="t('settings.btMaxDownloads')"
              :info-tooltip="t('settings.btMaxDownloadsHint')"
            >
              <UiTextField type="number" v-model="draft.bt.maxDownloads" :min="1" :max="1000" />
            </SettingsField>

            <SettingsField
              :label="t('settings.btMaxSeeds')"
              :info-tooltip="t('settings.btMaxSeedsHint')"
            >
              <UiTextField type="number" v-model="draft.bt.maxSeeds" :min="0" :max="1000" />
            </SettingsField>

            <SettingsField
              :label="t('settings.btMaxTorrents')"
              :info-tooltip="t('settings.btMaxTorrentsHint')"
            >
              <UiTextField type="number" v-model="draft.bt.maxTorrents" :min="1" :max="10000" />
            </SettingsField>

            <SettingsField
              :label="t('settings.btActiveLimit')"
              :info-tooltip="t('settings.btActiveLimitHint')"
            >
              <UiTextField type="number" v-model="draft.bt.activeLimit" :min="1" :max="10000" />
            </SettingsField>
          </div>
        </div>
      </div>
      <!-- 速率限制 (both engines) -->
      <div class="bt-subgroup" :class="{ 'bt-subgroup--open': rateLimitOpen }">
        <button
          type="button"
          class="bt-subgroup__header"
          :aria-expanded="rateLimitOpen"
          @click="toggleRateLimit"
        >
          <span
            class="i-ri-arrow-down-s-line bt-subgroup__chevron"
            :class="{ 'bt-subgroup__chevron--open': rateLimitOpen }"
            aria-hidden="true"
          />
          <span class="bt-subgroup__title">{{ t("settings.btGroupRateLimit") }}</span>
        </button>
        <div v-show="rateLimitOpen" class="bt-subgroup__content">
          <div class="settings-grid">
            <SettingsField
              :label="t('settings.btGlobalDownloadRateLimit')"
              :hint="t('settings.btGlobalDownloadRateLimitHint')"
            >
              <UiTextField
                type="number"
                :model-value="draft.bt.globalDownloadRateLimit"
                :min="0"
                :step="1024"
                unit="B/s"
                @update:model-value="draft.bt.globalDownloadRateLimit = Number($event ?? 0)"
              />
            </SettingsField>

            <SettingsField
              :label="t('settings.btGlobalUploadRateLimit')"
              :hint="t('settings.btGlobalUploadRateLimitHint')"
            >
              <UiTextField
                type="number"
                :model-value="draft.bt.globalUploadRateLimit"
                :min="0"
                :step="1024"
                unit="B/s"
                @update:model-value="draft.bt.globalUploadRateLimit = Number($event ?? 0)"
              />
            </SettingsField>
          </div>
        </div>
      </div>
    </div>
  </SettingsSection>
</template>

<style scoped>
.bt-subgroups {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.bt-subgroup {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.bt-subgroup__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  margin: 0;
  border: none;
  background: transparent;
  color: var(--color-heading);
  font: inherit;
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.bt-subgroup__header:hover {
  background: var(--color-surface-muted);
}

.bt-subgroup__header:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px var(--color-focus-ring);
}

.bt-subgroup__chevron {
  flex: 0 0 auto;
  font-size: 1.1rem;
  transition: transform 0.2s ease;
}

.bt-subgroup__chevron--open {
  transform: rotate(180deg);
}

.bt-subgroup__title {
  font-weight: var(--font-weight-semibold);
}

.bt-subgroup__content {
  padding: var(--space-4);
  border-top: 1px solid var(--color-border);
}
</style>
