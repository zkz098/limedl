// ── Re-exported generated types (single source of truth from Rust) ──
export type {
  CdnTestPhase,
  CdnTestProgress,
  DefaultNodeResult,
  SpeedTestResult,
} from "./generated/types";

// ── Tauri-layer composite types (defined in src-tauri, not limedl-core) ──

/** A single candidate IP from the CDN speed test. */
export type SpeedTestCandidate = import("./generated/types").SpeedTestResult;

/** Progress counter for a CDN test phase. */
export interface PhaseProgress {
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
  defaultNode: import("./generated/types").DefaultNodeResult | null;
}
