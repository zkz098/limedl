import type { AdaptiveProfile, ChecksumMode } from "./download";

export type { AdaptiveProfile };

export type ProxyMode = "disabled" | "system" | "manual";
export type SchedulerMode = "traditional" | "automatic";
export type ChunkSizeStrategy = "fixed" | "adaptive";
export type ThemeColor = "amber" | "sky" | "lime";
export type BackgroundOpacityPreset = "default" | "acrylic" | "frosted";
export type ColorMode = "light" | "dark" | "system";
export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

export type SortKey = "name" | "size" | "progress" | "speed" | "added_at" | "state";
export type SortDirection = "asc" | "desc";

export interface AppearanceSettings {
  themeColor: ThemeColor;
  backgroundOpacity: BackgroundOpacityPreset;
  colorMode: ColorMode;
  showDetailInfo: boolean;
  sortKey: SortKey;
  sortDirection: SortDirection;
  compactView: boolean;
  visibleColumns: string[];
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
  chunkSizeStrategy: ChunkSizeStrategy;
}

export interface DownloadDefaultsSettings {
  defaultDownloadDir: string;
  defaultMaxRetries: number;
  defaultChecksum: ChecksumMode;
  defaultUserAgent: string;
}

export type BtPreallocateMode = "none" | "full";
export type BtEncryptionMode = "enabled" | "disabled" | "forced";
export interface BtSettings {
  pauseUploadWhenLimitReached: boolean;
  uploadLimitBytes: number;
  uploadRatioLimit: number;
  dhtEnabled: boolean;
  trackerList: string;
  trackerListUrl: string;
  /** TCP listen port. null = OS assigns. */
  listenPort: number | null;
  /** Port range for TCP listen. null = any. */
  listenPortRange: { start: number; end: number } | null;
  /** Enable UPnP IGD port mapping. */
  upnpEnabled: boolean;
  /** Enable NAT-PMP/PCP port mapping. */
  enableNatpmp: boolean;
  /** Enable IPv6 dual-stack. */
  enableIpv6: boolean;
  /** Peer Exchange BEP 11. */
  enablePex: boolean;
  /** Local Service Discovery BEP 14. */
  enableLsd: boolean;
  /** µTP BEP 29. */
  enableUtp: boolean;
  /** Fast Extension BEP 6. */
  enableFastExtension: boolean;
  /** Holepunch BEP 55. */
  enableHolepunch: boolean;
  /** HTTP Web Seed. */
  enableWebSeed: boolean;
  /** Super seeding BEP 16. */
  enableSuperSeeding: boolean;
  /** Global download rate limit in bytes/sec. 0 = unlimited. */
  globalDownloadRateLimit: number;
  /** Global upload rate limit in bytes/sec. 0 = unlimited. */
  globalUploadRateLimit: number;
  /** File preallocation strategy. */
  preallocateMode: BtPreallocateMode;
  /** Protocol encryption (MSE/PE) mode. */
  encryptionMode: BtEncryptionMode;
  /** Max auto-managed active downloads. */
  maxDownloads: number;
  /** Max auto-managed active seeds. */
  maxSeeds: number;
  /** Max total torrents. */
  maxTorrents: number;
  /** Hard limit on total active torrents. */
  activeLimit: number;
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

export interface CdnAccelerationSettings {
  enabled: boolean;
  activeIp: string | null;
  activeSpeedMbps: number | null;
  lastTestAtMs: number | null;
  lastError: string | null;
}

export interface MirrorEntry {
  url: string;
  enabled: boolean;
  order: number;
  /** Frontend-only unique ID for stable v-for keys. Not serialized by backend. */
  _uid?: number;
}

export interface GitHubMirrorSettings {
  enabled: boolean;
  mirrors: MirrorEntry[];
}

export interface NotificationSettings {
  enabled: boolean;
}

export type DiskType = "ssd" | "hdd";

export interface IoBaselineSettings {
  bufferLimitMb: number;
  gameModeBufferMb: number;
  gameMode: boolean;
  diskTypeOverrides: Record<string, DiskType>;
  maxParallelHdd: number;
  gameModeMaxParallel: number;
}

export interface AppSettings {
  globalSpeedLimitBps: number;
  appearance: AppearanceSettings;
  proxy: ProxySettings;
  scheduler: SchedulerSettings;
  download: DownloadDefaultsSettings;
  bt: BtSettings;
  logging: LogSettings;
  aria2Rpc: Aria2RpcSettings;
  cdnAcceleration: CdnAccelerationSettings;
  githubMirror: GitHubMirrorSettings;
  notifications: NotificationSettings;
  ioBaseline: IoBaselineSettings;
}
