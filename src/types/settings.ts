// ── Re-exported generated types (single source of truth from Rust) ──
export type {
  AdaptiveProfile,
  AppearanceSettings,
  AutomaticSchedulerSettings,
  BackgroundOpacityPreset,
  BtEncryptionMode,
  BtPortRange,
  BtPreallocateMode,
  BtSettings,
  CdnAccelerationSettings,
  ChecksumMode,
  ChunkSizeStrategy,
  CloseBehavior,
  ColorMode,
  DiskType,
  DownloadDefaultsSettings,
  DoubleClickOnCompleted,
  DoubleClickOnUncompleted,
  DoubleClickSettings,
  LogLevel,
  LogSettings,
  NotificationSettings,
  PetSettings,
  ProxyMode,
  ProxySettings,
  SchedulerMode,
  SchedulerSettings,
  SortDirection,
  SortKey,
  ThemeColor,
  TraditionalSchedulerSettings,
} from "./generated/types";

// ── Import generated types for local extension ──
import type {
  AppSettings as GeneratedAppSettings,
  Aria2RpcSettings as GeneratedAria2RpcSettings,
  IoBaselineSettings as GeneratedIoBaselineSettings,
  MirrorEntry as GeneratedMirrorEntry,
  PetSettings as GeneratedPetSettings,
} from "./generated/types";

/** Aria2RpcSettings as generated from Rust. */
export type Aria2RpcSettings = GeneratedAria2RpcSettings;

/** IoBaselineSettings with frontend-only gameMode field (runtime-only, never persisted). */
export interface IoBaselineSettings extends GeneratedIoBaselineSettings {
  /** Whether game/performance mode is currently active (runtime-only, never persisted by backend). */
  gameMode?: boolean;
}

/** MirrorEntry with frontend-only _uid field. */
export interface MirrorEntry extends GeneratedMirrorEntry {
  /** Frontend-only unique ID for stable v-for keys. Not serialized by backend. */
  _uid?: number;
}

/** GitHubMirrorSettings using extended MirrorEntry. */
export interface GitHubMirrorSettings {
  enabled: boolean;
  mirrors: MirrorEntry[];
}

/**
 * AppSettings with frontend-extended sub-types.
 * Generated counterpart is in generated/types.ts.
 */
export type AppSettings = Omit<
  GeneratedAppSettings,
  "ioBaseline" | "githubMirror" | "aria2Rpc" | "pet"
> & {
  ioBaseline: IoBaselineSettings;
  githubMirror: GitHubMirrorSettings;
  aria2Rpc: Aria2RpcSettings;
  pet: GeneratedPetSettings;
};

export const DEFAULT_PET_SETTINGS: GeneratedPetSettings = {
  enabled: false,
  scale: 1,
  opacity: 1,
  keepAliveWhenMainHidden: true,
  model: "default",
};
