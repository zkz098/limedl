/** A single candidate IP from the CDN speed test. */
export interface SpeedTestCandidate {
  ip: string;
  tcpLatencyMs: number;
  throughputMbps: number | null;
  error: string | null;
}

/** Baseline measurement of the default DNS-resolved node (no IP override). */
export interface DefaultNodeResult {
  ip: string | null;
  tcpLatencyMs: number;
  throughputMbps: number | null;
  error: string | null;
}

/** Progress counter for a CDN test phase. */
export interface PhaseProgress {
  current: number;
  total: number;
}

/** Progress event payload emitted via the `cdn-test-progress` Tauri event. */
export interface CdnTestProgress {
  /** Phase name in camelCase: "fetchingRanges" | "screening" | "measuringThroughput". */
  phase: "fetchingRanges" | "screening" | "measuringThroughput";
  current: number;
  total: number;
}

export interface CdnDetail {
  state: string;
  activeIp: string | null;
  activeSpeedMbps: number | null;
  /** Current test phase (PascalCase): "FetchingRanges" | "Screening" | "MeasuringThroughput", or null when idle. */
  phase: string | null;
  /** Progress for the current phase, or null when not testing. */
  phaseProgress: PhaseProgress | null;
  /** All candidate IPs from the most recent speed test. */
  candidates: SpeedTestCandidate[];
  defaultNode: DefaultNodeResult | null;
}
