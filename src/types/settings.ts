import type { ChecksumMode } from "./download";

export type ProxyMode = "disabled" | "system" | "manual";
export type SchedulerMode = "traditional" | "automatic";
export type AdaptiveProfile = "conservative" | "balanced" | "aggressive";
export type DeviceLearningMode = "fixed" | "mobile" | "semi_mobile";

export interface ProxySettings {
  mode: ProxyMode;
  manualUrl: string;
}

export interface TraditionalSchedulerSettings {
  maxParallelTasks: number;
}

export interface AutomaticSchedulerSettings {
  maxParallelThreads: number;
  maxThreadsPerTask: number;
  adaptiveProfile: AdaptiveProfile;
}

export interface SchedulerSettings {
  mode: SchedulerMode;
  traditional: TraditionalSchedulerSettings;
  automatic: AutomaticSchedulerSettings;
}

export interface DownloadDefaultsSettings {
  defaultDownloadDir: string;
  defaultMaxRetries: number;
  defaultChecksum: ChecksumMode;
}

export interface NetworkLearningMetrics {
  estimatedBandwidthBps: number;
  stabilityScore: number;
  penaltyRate: number;
  recommendedInitialThreads: number;
  recommendedMaxThreadsPerTaskCap: number;
  sampleCount: number;
  lastObservedAtMs: number;
}

export interface NetworkSceneProfile {
  id: string;
  name: string;
  learningEnabled: boolean;
  learnedMetrics: NetworkLearningMetrics | null;
  updatedAtMs: number;
}

export interface NetworkLearningSettings {
  deviceMode: DeviceLearningMode;
  currentSceneId: string;
  scenes: NetworkSceneProfile[];
}

export interface AppSettings {
  proxy: ProxySettings;
  scheduler: SchedulerSettings;
  download: DownloadDefaultsSettings;
  networkLearning: NetworkLearningSettings;
}
