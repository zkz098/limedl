<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import UiButton from "../ui/UiButton.vue";
import UiCard from "../ui/UiCard.vue";
import UiBadge from "../ui/UiBadge.vue";
import UiProgress from "../ui/UiProgress.vue";
import UiInput from "../ui/UiInput.vue";
import type { AppSettings } from "../../types/settings";
import {
  testAcceleration,
  cancelAcceleration,
  clearAcceleration,
  getAccelerationDetail,
  fetchCloudflareRanges,
  applyAcceleration,
} from "../../lib/tauri/cdn-api";
import type {
  CdnDetail,
  SpeedTestCandidate,
  CdnTestProgress,
  PhaseProgress,
  DefaultNodeResult,
} from "../../lib/tauri/cdn-api";
import { listen } from "@tauri-apps/api/event";
import { useIntervalFn } from "@vueuse/core";

const props = defineProps<{
  draft: AppSettings;
  t: (key: string, options?: Record<string, unknown>) => string;
}>();

// ── State ──
const testing = ref(false);
const phase = ref<string | null>(null);
const phaseProgress = ref<PhaseProgress | null>(null);
const candidates = ref<SpeedTestCandidate[]>([]);
const defaultNode = ref<DefaultNodeResult | null>(null);
const showAdvanced = ref(false);
const showCandidates = ref(true);
const cloudflareRanges = ref<string[]>([]);
const rangesLoaded = ref(false);
const manualIp = ref("");
const manualIpError = ref<string | null>(null);

// ── Phase mapping (PascalCase from CdnDetail, camelCase from CdnTestProgress) ──
const PHASE_KEY_MAP: Record<string, string> = {
  FetchingRanges: "phaseFetchingRanges",
  Screening: "phaseScreening",
  MeasuringThroughput: "phaseMeasuring",
  fetchingRanges: "phaseFetchingRanges",
  screening: "phaseScreening",
  measuringThroughput: "phaseMeasuring",
};

// ── Computed ──
const statusType = computed<"idle" | "testing" | "ready" | "error">(() => {
  const cdn = props.draft.cdnAcceleration;
  if (cdn.lastError) return "error";
  if (cdn.activeIp != null && cdn.activeSpeedMbps != null) return "ready";
  if (testing.value) return "testing";
  return "idle";
});

const statusBadgeTone = computed<"neutral" | "info" | "success" | "danger">(() => {
  const map: Record<string, "neutral" | "info" | "success" | "danger"> = {
    idle: "neutral",
    testing: "info",
    ready: "success",
    error: "danger",
  };
  return map[statusType.value];
});

const statusBadgeLabel = computed(() => {
  const keyMap: Record<string, string> = {
    idle: "statusIdle",
    testing: "statusTesting",
    ready: "statusReady",
    error: "statusError",
  };
  return props.t(`settings.cdnAcceleration.${keyMap[statusType.value]}`);
});

const hasResult = computed(() => props.draft.cdnAcceleration.activeIp != null);

const lastTestTime = computed(() => {
  const ms = props.draft.cdnAcceleration.lastTestAtMs;
  if (ms == null) return null;
  return new Date(ms).toLocaleString();
});

const progressPercent = computed(() => {
  if (!phaseProgress.value) return 0;
  const { current, total } = phaseProgress.value;
  if (total <= 0) return 0;
  return (current / total) * 100;
});

const phaseLabel = computed(() => {
  if (!phase.value) return "";
  const key = PHASE_KEY_MAP[phase.value];
  return key ? props.t(`settings.cdnAcceleration.${key}`) : phase.value;
});

const progressLabel = computed(() => {
  if (!phaseProgress.value) return "";
  const { current, total } = phaseProgress.value;
  return `${current} / ${total}`;
});

const activeIp = computed(() => props.draft.cdnAcceleration.activeIp);

const activeSpeedFormatted = computed(() => {
  const s = props.draft.cdnAcceleration.activeSpeedMbps;
  if (s == null) return "-";
  return s.toFixed(2);
});

function fmtSpeed(mbps: number | null): string {
  if (mbps == null) return "-";
  return mbps.toFixed(2);
}

function fmtLatency(ms: number): string {
  return ms.toFixed(2);
}

const bestLatency = computed(() => {
  if (candidates.value.length === 0) return null;
  return Math.min(...candidates.value.map((c) => c.tcpLatencyMs));
});

const speedImprovement = computed(() => {
  const best = props.draft.cdnAcceleration.activeSpeedMbps;
  const baseline = defaultNode.value?.throughputMbps;
  if (best == null || baseline == null || baseline <= 0) return null;
  return ((best - baseline) / baseline) * 100;
});

const latencyImprovement = computed(() => {
  const best = bestLatency.value;
  const baseline = defaultNode.value?.tcpLatencyMs;
  if (best == null || baseline == null || baseline <= 0) return null;
  return ((best - baseline) / baseline) * 100;
});

function fmtImprovement(pct: number | null): string {
  if (pct == null) return "-";
  const sign = pct >= 0 ? "+" : "";
  return `${sign}${pct.toFixed(1)}%`;
}

// ── Update from detail ──
function updateFromDetail(detail: CdnDetail): void {
  const cdn = props.draft.cdnAcceleration;

  testing.value = detail.state === "Testing";
  phase.value = detail.phase;
  phaseProgress.value = detail.phaseProgress;
  candidates.value = detail.candidates;
  defaultNode.value = detail.defaultNode;

  if (detail.state === "Ready") {
    cdn.activeIp = detail.activeIp;
    cdn.activeSpeedMbps = detail.activeSpeedMbps;
    cdn.lastTestAtMs = Date.now();
    cdn.lastError = null;
    pausePolling();
  } else if (detail.state.startsWith("Error:")) {
    cdn.lastError = detail.state.slice(7);
    cdn.lastTestAtMs = Date.now();
    pausePolling();
  }
}

// ── Actions ──
async function startTest(): Promise<void> {
  testing.value = true;
  phase.value = null;
  phaseProgress.value = null;
  candidates.value = [];
  defaultNode.value = null;
  try {
    await testAcceleration();
  } catch (e) {
    console.error("Failed to start CDN test:", e);
    testing.value = false;
  }
}

async function cancelTest(): Promise<void> {
  try {
    await cancelAcceleration();
  } catch (e) {
    console.error("Failed to cancel CDN test:", e);
  }
  testing.value = false;
  phase.value = null;
  phaseProgress.value = null;
}

async function clearResult(): Promise<void> {
  try {
    await clearAcceleration();
  } catch (e) {
    console.error("Failed to clear CDN acceleration:", e);
  }
  const cdn = props.draft.cdnAcceleration;
  cdn.activeIp = null;
  cdn.activeSpeedMbps = null;
  cdn.lastTestAtMs = null;
  cdn.lastError = null;
  testing.value = false;
  phase.value = null;
  phaseProgress.value = null;
  candidates.value = [];
  defaultNode.value = null;
}

async function applyCandidate(ip: string, speedMbps: number): Promise<void> {
  try {
    await applyAcceleration(ip, speedMbps);
    const cdn = props.draft.cdnAcceleration;
    cdn.activeIp = ip;
    cdn.activeSpeedMbps = speedMbps;
    cdn.lastTestAtMs = Date.now();
    cdn.lastError = null;
  } catch (e) {
    console.error("Failed to apply acceleration:", e);
  }
}

async function toggleAdvanced(): Promise<void> {
  showAdvanced.value = !showAdvanced.value;
  if (showAdvanced.value && !rangesLoaded.value) {
    try {
      cloudflareRanges.value = await fetchCloudflareRanges();
      rangesLoaded.value = true;
    } catch (e) {
      console.error("Failed to fetch Cloudflare ranges:", e);
    }
  }
}

function applyManualIp(): void {
  manualIpError.value = null;
  const ip = manualIp.value.trim();
  if (!ip) return;

  const ipv4Regex = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;
  const match = ipv4Regex.exec(ip);
  if (!match) {
    manualIpError.value = props.t("settings.cdnAcceleration.manualIpInvalid");
    return;
  }
  const octets = [match[1], match[2], match[3], match[4]].map(Number);
  if (octets.some((o) => o > 255)) {
    manualIpError.value = props.t("settings.cdnAcceleration.manualIpInvalid");
    return;
  }

  applyAcceleration(ip, 0)
    .then(() => {
      const cdn = props.draft.cdnAcceleration;
      cdn.activeIp = ip;
      cdn.activeSpeedMbps = 0;
      cdn.lastTestAtMs = Date.now();
      cdn.lastError = null;
      manualIp.value = "";
      manualIpError.value = null;
    })
    .catch((e) => {
      console.error("Failed to apply manual IP:", e);
    });
}

// Clear manualIp error on input
watch(manualIp, () => {
  if (manualIpError.value) manualIpError.value = null;
});

// ── Polling (2s interval) ──
const POLL_INTERVAL_MS = 2000;
const { pause: pausePolling, resume: resumePolling } = useIntervalFn(
  async () => {
    try {
      const detail = await getAccelerationDetail();
      updateFromDetail(detail);
    } catch (e) {
      console.error("Failed to poll CDN status:", e);
    }
  },
  POLL_INTERVAL_MS,
  { immediate: false },
);

watch(testing, (val) => {
  if (val) resumePolling();
  else pausePolling();
});

// ── Event listeners ──
let unlistenComplete: (() => void) | undefined;
let unlistenProgress: (() => void) | undefined;

onMounted(async () => {
  unlistenComplete = await listen<CdnDetail>("cdn-test-complete", async () => {
    try {
      const detail = await getAccelerationDetail();
      updateFromDetail(detail);
    } catch (e) {
      console.error("Failed to fetch final CDN detail:", e);
      testing.value = false;
      phase.value = null;
      phaseProgress.value = null;
      pausePolling();
    }
  });

  unlistenProgress = await listen<CdnTestProgress>(
    "cdn-test-progress",
    (event) => {
      const { phase: evPhase, current, total } = event.payload;
      phase.value = evPhase;
      phaseProgress.value = { current, total };
    },
  );
});

onUnmounted(() => {
  pausePolling();
  unlistenComplete?.();
  unlistenProgress?.();
});
</script>

<template>
  <UiCard>
    <template #header>
      <div class="cdn-panel__header">
        <div>
          <p class="section-kicker">{{ t("settings.cdnAcceleration.title") }}</p>
          <p class="panel-title">{{ t("settings.cdnAcceleration.description") }}</p>
        </div>
        <UiBadge :tone="statusBadgeTone" size="sm">{{ statusBadgeLabel }}</UiBadge>
      </div>
    </template>

    <!-- Enable toggle -->
    <label class="settings-field">
      <span class="settings-field__label">{{ t("settings.cdnAcceleration.enable") }}</span>
      <button
        type="button"
        class="settings-toggle"
        :class="{ 'settings-toggle--active': draft.cdnAcceleration.enabled }"
        :aria-pressed="draft.cdnAcceleration.enabled"
        @click="draft.cdnAcceleration.enabled = !draft.cdnAcceleration.enabled"
      >
        <span
          class="settings-toggle__icon"
          :class="draft.cdnAcceleration.enabled ? 'i-ri-checkbox-circle-fill' : 'i-ri-checkbox-blank-circle-line'"
          aria-hidden="true"
        />
        <span class="settings-toggle__text">{{ t("settings.cdnAcceleration.enable") }}</span>
      </button>
    </label>

    <!-- Staged progress -->
    <div v-show="testing || phase" class="cdn-panel__progress">
      <div class="cdn-panel__progress-header">
        <span class="cdn-panel__phase-icon i-ri-loader-4-line cdn-panel__spin" aria-hidden="true" />
        <span v-if="phaseLabel" class="cdn-panel__phase-label">{{ phaseLabel }}</span>
      </div>
      <UiProgress :value="progressPercent" show-label :label="progressLabel" />
    </div>

    <!-- Action buttons -->
    <div class="cdn-panel__actions">
      <UiButton
        variant="primary"
        :icon="testing ? undefined : 'i-ri-rocket-2-line'"
        :loading="testing"
        :disabled="testing"
        @click="startTest"
      >
        {{ hasResult ? t("settings.cdnAcceleration.testAgain") : t("settings.cdnAcceleration.triggerButton") }}
      </UiButton>
      <UiButton
        v-if="testing"
        variant="secondary"
        icon="i-ri-stop-circle-line"
        @click="cancelTest"
      >
        {{ t("settings.cdnAcceleration.cancelButton") }}
      </UiButton>
      <UiButton
        v-if="statusType === 'ready'"
        variant="ghost"
        icon="i-ri-eraser-line"
        @click="clearResult"
      >
        {{ t("settings.cdnAcceleration.clearButton") }}
      </UiButton>
    </div>

    <!-- Data warning -->
    <p class="cdn-panel__hint">{{ t("settings.cdnAcceleration.dataWarning") }}</p>

    <!-- Error display -->
    <div v-if="statusType === 'error'" class="cdn-panel__error">
      <span class="i-ri-error-warning-line" aria-hidden="true" />
      <span class="cdn-panel__error-msg">{{ draft.cdnAcceleration.lastError }}</span>
      <UiButton variant="secondary" size="sm" @click="startTest">
        {{ t("settings.cdnAcceleration.testAgain") }}
      </UiButton>
    </div>

    <!-- Result card -->
    <div v-if="statusType === 'ready' && hasResult" class="cdn-panel__result">
      <div class="cdn-panel__result-grid">
        <div class="cdn-panel__result-item">
          <span class="cdn-panel__result-key">{{ t("settings.cdnAcceleration.bestIp") }}</span>
          <span class="cdn-panel__result-value cdn-panel__mono">{{ draft.cdnAcceleration.activeIp }}</span>
        </div>
        <div class="cdn-panel__result-item">
          <span class="cdn-panel__result-key">{{ t("settings.cdnAcceleration.speedMbps") }}</span>
          <span class="cdn-panel__result-value">{{ activeSpeedFormatted }} MB/s</span>
        </div>
        <div v-if="lastTestTime" class="cdn-panel__result-item">
          <span class="cdn-panel__result-key">{{ t("settings.cdnAcceleration.testedAt") }}</span>
          <span class="cdn-panel__result-value">{{ lastTestTime }}</span>
        </div>
      </div>
      <div v-if="speedImprovement != null || latencyImprovement != null" class="cdn-panel__speedup">
        <div class="cdn-panel__speedup-item">
          <span class="cdn-panel__speedup-key">{{ t("settings.cdnAcceleration.speedupSpeed") }}</span>
          <span
            class="cdn-panel__speedup-value"
            :class="{ 'cdn-panel__speedup-value--positive': (speedImprovement ?? 0) > 0 }"
          >{{ fmtImprovement(speedImprovement) }}</span>
        </div>
        <div class="cdn-panel__speedup-item">
          <span class="cdn-panel__speedup-key">{{ t("settings.cdnAcceleration.speedupLatency") }}</span>
          <span
            class="cdn-panel__speedup-value"
            :class="{ 'cdn-panel__speedup-value--positive': (latencyImprovement ?? 0) > 0 }"
          >{{ fmtImprovement(latencyImprovement) }}</span>
        </div>
        <span class="cdn-panel__speedup-baseline">{{ t("settings.cdnAcceleration.vsDefault") }}</span>
      </div>
    </div>

    <!-- Candidate nodes table -->
    <div v-if="candidates.length > 0" class="cdn-panel__candidates">
      <div class="cdn-panel__candidates-header">
        <h4 class="cdn-panel__candidates-title">{{ t("settings.cdnAcceleration.candidatesTitle") }}</h4>
        <button
          type="button"
          class="cdn-panel__collapse-btn"
          :aria-expanded="showCandidates"
          @click="showCandidates = !showCandidates"
        >
          <span
            :class="showCandidates ? 'i-ri-arrow-up-s-line' : 'i-ri-arrow-down-s-line'"
            aria-hidden="true"
          />
        </button>
      </div>
      <div v-show="showCandidates" class="cdn-panel__candidates-body">
        <table class="cdn-panel__table">
          <thead>
            <tr>
              <th>{{ t("settings.cdnAcceleration.candidateIp") }}</th>
              <th>{{ t("settings.cdnAcceleration.candidateLatency") }}</th>
              <th>{{ t("settings.cdnAcceleration.candidateThroughput") }}</th>
              <th>{{ t("settings.cdnAcceleration.candidateStatus") }}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="c in candidates"
              :key="c.ip"
              :class="{ 'cdn-panel__row--active': c.ip === activeIp }"
            >
              <td class="cdn-panel__mono">{{ c.ip }}</td>
              <td>{{ fmtLatency(c.tcpLatencyMs) }} ms</td>
              <td>{{ c.throughputMbps != null ? `${fmtSpeed(c.throughputMbps)} MB/s` : "-" }}</td>
              <td>
                <UiBadge v-if="c.ip === activeIp" tone="success" size="sm">
                  {{ t("settings.cdnAcceleration.candidateActive") }}
                </UiBadge>
                <UiBadge v-else-if="c.throughputMbps === null || c.error" tone="warning" size="sm">
                  {{ t("settings.cdnAcceleration.candidateFailed") }}
                </UiBadge>
              </td>
              <td>
                <UiButton
                  v-if="c.ip !== activeIp && c.throughputMbps !== null"
                  variant="secondary"
                  size="sm"
                  :disabled="testing"
                  @click="applyCandidate(c.ip, c.throughputMbps ?? 0)"
                >
                  {{ t("settings.cdnAcceleration.candidateApply") }}
                </UiButton>
              </td>
            </tr>
          </tbody>
          <tfoot v-if="defaultNode">
            <tr class="cdn-panel__row--default">
              <td class="cdn-panel__mono">{{ defaultNode.ip ?? "-" }}</td>
              <td>{{ fmtLatency(defaultNode.tcpLatencyMs) }} ms</td>
              <td>{{ defaultNode.throughputMbps != null ? `${fmtSpeed(defaultNode.throughputMbps)} MB/s` : "-" }}</td>
              <td>
                <UiBadge tone="neutral" size="sm">
                  {{ t("settings.cdnAcceleration.defaultNode") }}
                </UiBadge>
              </td>
              <td />
            </tr>
          </tfoot>
        </table>
      </div>
    </div>

    <p v-if="candidates.length === 0 && !testing" class="cdn-panel__hint">
      {{ t("settings.cdnAcceleration.noCandidates") }}
    </p>

    <!-- Advanced section -->
    <div class="cdn-panel__advanced">
      <button
        type="button"
        class="cdn-panel__advanced-toggle"
        :aria-expanded="showAdvanced"
        @click="toggleAdvanced"
      >
        <span class="i-ri-settings-3-line cdn-panel__advanced-toggle-icon" aria-hidden="true" />
        <span class="cdn-panel__advanced-toggle-label">{{ t("settings.cdnAcceleration.advancedSection") }}</span>
        <span
          :class="showAdvanced ? 'i-ri-arrow-up-s-line' : 'i-ri-arrow-down-s-line'"
          aria-hidden="true"
        />
      </button>
      <div v-show="showAdvanced" class="cdn-panel__advanced-body">
        <!-- Cloudflare IP Ranges -->
        <div class="cdn-panel__advanced-block">
          <h5 class="cdn-panel__advanced-title">{{ t("settings.cdnAcceleration.cloudflareRanges") }}</h5>
          <div v-if="rangesLoaded" class="cdn-panel__ranges-list">
            <code
              v-for="range in cloudflareRanges"
              :key="range"
              class="cdn-panel__mono cdn-panel__range-chip"
            >{{ range }}</code>
          </div>
          <p v-else class="cdn-panel__hint">
            <span class="i-ri-loader-4-line cdn-panel__spin" aria-hidden="true" />
            Loading ranges...
          </p>
        </div>

        <!-- Manual IP Apply -->
        <div class="cdn-panel__advanced-block">
          <h5 class="cdn-panel__advanced-title">{{ t("settings.cdnAcceleration.manualApply") }}</h5>
          <p class="cdn-panel__hint">{{ t("settings.cdnAcceleration.manualApplyHint") }}</p>
          <div class="cdn-panel__manual-row">
            <div class="cdn-panel__manual-input">
              <UiInput
                v-model="manualIp"
                placeholder="e.g. 104.16.0.1"
                :disabled="testing"
              />
            </div>
            <UiButton
              variant="secondary"
              size="sm"
              :disabled="testing || !manualIp.trim()"
              @click="applyManualIp"
            >
              {{ t("settings.cdnAcceleration.manualApplyButton") }}
            </UiButton>
          </div>
          <p v-if="manualIpError" class="cdn-panel__error-text">{{ manualIpError }}</p>
        </div>
      </div>
    </div>
  </UiCard>
</template>

<style scoped>
/* ── Header ── */
.cdn-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

/* ── Progress ── */
.cdn-panel__progress {
  margin-top: var(--space-5);
}

.cdn-panel__progress-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.cdn-panel__phase-icon {
  font-size: 1rem;
  color: var(--color-accent-strong);
}

.cdn-panel__phase-label {
  font-size: var(--font-size-small);
  color: var(--color-text-muted);
}

.cdn-panel__spin {
  animation: cdn-spin 0.8s linear infinite;
}

@keyframes cdn-spin {
  to { transform: rotate(360deg); }
}

/* ── Actions ── */
.cdn-panel__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-4);
  flex-wrap: wrap;
}

/* ── Hint ── */
.cdn-panel__hint {
  margin: var(--space-2) 0 0;
  font-size: var(--font-size-micro);
  color: var(--color-text-muted);
}

/* ── Error ── */
.cdn-panel__error {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-top: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--color-danger-bg);
  border: 1px solid var(--color-danger-border);
  color: var(--color-danger-text);
  font-size: var(--font-size-small);
}

.cdn-panel__error-msg {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}

.cdn-panel__error-text {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-micro);
  color: var(--color-danger-text);
}

/* ── Result ── */
.cdn-panel__result {
  margin-top: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--color-panel-muted);
  border: 1px solid var(--color-border);
}

.cdn-panel__result-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: var(--space-3);
}

.cdn-panel__result-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.cdn-panel__result-key {
  font-size: var(--font-size-label);
  color: var(--color-text-muted);
}

.cdn-panel__result-value {
  color: var(--color-text-main);
  font-weight: 600;
}

.cdn-panel__speedup {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: 1px solid var(--color-border);
  flex-wrap: wrap;
}

.cdn-panel__speedup-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.cdn-panel__speedup-key {
  font-size: var(--font-size-label);
  color: var(--color-text-muted);
}

.cdn-panel__speedup-value {
  font-size: var(--font-size-metric);
  font-weight: 700;
  color: var(--color-text-main);
}

.cdn-panel__speedup-value--positive {
  color: var(--color-success-text);
}

.cdn-panel__speedup-baseline {
  font-size: var(--font-size-micro);
  color: var(--color-text-soft);
  margin-left: auto;
}

/* ── Candidates ── */
.cdn-panel__candidates {
  margin-top: var(--space-5);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.cdn-panel__candidates-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  background: var(--color-panel-muted);
}

.cdn-panel__candidates-title {
  margin: 0;
  font-size: var(--font-size-small);
  font-weight: 600;
  color: var(--color-heading);
}

.cdn-panel__candidates-body {
  overflow-x: auto;
}

/* ── Collapse button ── */
.cdn-panel__collapse-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  font-size: 1.1rem;
  transition: background-color 0.2s ease;
}

.cdn-panel__collapse-btn:hover {
  background: var(--color-surface-hover);
}

/* ── Table ── */
.cdn-panel__table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-small);
}

.cdn-panel__table th,
.cdn-panel__table td {
  padding: var(--space-2) var(--space-4);
  text-align: left;
  border-bottom: 1px solid var(--color-border);
  white-space: nowrap;
}

.cdn-panel__table th {
  font-size: var(--font-size-label);
  font-weight: 600;
  color: var(--color-text-muted);
  background: var(--color-surface-muted);
}

.cdn-panel__table tbody tr:hover {
  background: var(--color-surface-hover);
}

.cdn-panel__table tbody tr:last-child td {
  border-bottom: none;
}

.cdn-panel__row--active {
  background: var(--color-accent-soft);
}

.cdn-panel__row--active:hover {
  background: var(--color-accent-soft) !important;
}

.cdn-panel__row--default {
  background: var(--color-surface-muted);
  font-style: italic;
  color: var(--color-text-muted);
}

.cdn-panel__row--default:hover {
  background: var(--color-surface-muted) !important;
}

.cdn-panel__table tfoot tr td {
  border-top: 2px solid var(--color-border-strong);
  border-bottom: none;
}

/* ── Monospace ── */
.cdn-panel__mono {
  font-family: "Cascadia Code", "Fira Code", "Consolas", monospace;
  font-size: var(--font-size-label);
}

/* ── Advanced section ── */
.cdn-panel__advanced {
  margin-top: var(--space-5);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.cdn-panel__advanced-toggle {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  background: var(--color-panel-muted);
  border: none;
  font: inherit;
  font-size: var(--font-size-small);
  font-weight: 600;
  color: var(--color-heading);
  cursor: pointer;
  text-align: left;
  transition: background-color 0.2s ease;
}

.cdn-panel__advanced-toggle:hover {
  background: var(--color-surface-hover);
}

.cdn-panel__advanced-toggle-icon {
  font-size: 1.1rem;
  color: var(--color-text-muted);
}

.cdn-panel__advanced-toggle-label {
  flex: 1;
}

.cdn-panel__advanced-body {
  padding: var(--space-4);
}

.cdn-panel__advanced-block {
  margin-bottom: var(--space-4);
}

.cdn-panel__advanced-block:last-child {
  margin-bottom: 0;
}

.cdn-panel__advanced-title {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-label);
  font-weight: 600;
  color: var(--color-text-main);
}

/* ── Ranges list ── */
.cdn-panel__ranges-list {
  max-height: 10rem;
  overflow-y: auto;
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  padding: var(--space-2);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-surface-muted);
}

.cdn-panel__range-chip {
  padding: 0.125rem var(--space-2);
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  font-size: var(--font-size-micro);
  color: var(--color-text-muted);
}

/* ── Manual IP row ── */
.cdn-panel__manual-row {
  display: flex;
  gap: var(--space-2);
  align-items: flex-start;
}

.cdn-panel__manual-input {
  flex: 1;
  min-width: 0;
}
</style>
