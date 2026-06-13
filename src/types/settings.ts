import type { AdaptiveProfile, ChecksumMode } from "./download";

export type { AdaptiveProfile };

export type ProxyMode = "disabled" | "system" | "manual";
export type SchedulerMode = "traditional" | "automatic";
export type DeviceLearningMode = "fixed" | "mobile" | "semi_mobile";
export type ThemeColor = "default" | "amber" | "sky" | "lime";
export type BackgroundOpacityPreset = "default" | "acrylic" | "frosted";
export type ColorMode = "light" | "dark" | "system";
export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

export interface AppearanceSettings {
  themeColor: ThemeColor;
  backgroundOpacity: BackgroundOpacityPreset;
  colorMode: ColorMode;
  showDetailInfo: boolean;
  showHeatmap: boolean;
}

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
  minThreadsPerTask: number;
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
  defaultUserAgent: string;
  enableMetalink: boolean;
  enableSftp: boolean;
}

export interface BtSettings {
  pauseUploadWhenLimitReached: boolean;
  uploadLimitBytes: number;
  uploadRatioLimit: number;
  dhtEnabled: boolean;
  pexEnabled: boolean;
  trackerList: string;
  trackerListUrl: string;
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

export interface LogSettings {
  enabled: boolean;
  level: LogLevel;
  filePath: string;
}

export interface Aria2RpcSettings {
  enabled: boolean;
  port: number;
  secret: string | null;
}

export interface AppSettings {
  appearance: AppearanceSettings;
  proxy: ProxySettings;
  scheduler: SchedulerSettings;
  download: DownloadDefaultsSettings;
  bt: BtSettings;
  networkLearning: NetworkLearningSettings;
  logging: LogSettings;
  aria2Rpc: Aria2RpcSettings;
}
