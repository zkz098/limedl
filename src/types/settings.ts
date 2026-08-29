// ── Re-exported generated types (single source of truth from Rust) ──
export type {
  AdaptiveProfile,
  AutomaticSchedulerSettings,
  BackgroundOpacityPreset,
  BtEncryptionMode,
  BtPortRange,
  BtPreallocateMode,
  BtSettings,
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
  MatchType,
  NotificationSettings,
  ProxyMode,
  ProxySettings,
  ReplacementMode,
  SchedulerMode,
  SchedulerSettings,
  SortDirection,
  SortKey,
  ThemeColor,
  TraditionalSchedulerSettings,
} from "./generated/types";

import type {
  AppearanceSettings as GeneratedAppearanceSettings,
  AppSettings as GeneratedAppSettings,
  Aria2RpcSettings as GeneratedAria2RpcSettings,
  CdnAccelerationSettings as GeneratedCdnAccelerationSettings,
  IoBaselineSettings as GeneratedIoBaselineSettings,
  RewriteTarget as GeneratedRewriteTarget,
  UrlRewriteRule as GeneratedUrlRewriteRule,
  UrlRewriteSettings as GeneratedUrlRewriteSettings,
} from "./generated/types";

/** AppearanceSettings with optional language for backward compatibility. */
export interface AppearanceSettings extends Omit<GeneratedAppearanceSettings, "language"> {
  language?: string;
}

/** Aria2RpcSettings as generated from Rust. */
export type Aria2RpcSettings = GeneratedAria2RpcSettings;

/** CdnAccelerationSettings with optional provider fields. */
export interface CdnAccelerationSettings extends Omit<
  GeneratedCdnAccelerationSettings,
  "provider" | "customTestUrl" | "customCidrs"
> {
  provider?: string;
  customTestUrl?: string | null;
  customCidrs?: string | null;
}

/** IoBaselineSettings with frontend-only gameMode field (runtime-only, never persisted). */
export interface IoBaselineSettings extends GeneratedIoBaselineSettings {
  /** Whether game/performance mode is currently active (runtime-only, never persisted by backend). */
  gameMode?: boolean;
}

/** RewriteTarget with frontend-only _uid field. */
export interface RewriteTarget extends GeneratedRewriteTarget {
  /** Frontend-only unique ID for stable v-for keys. Not serialized by backend. */
  _uid?: number;
}

/** UrlRewriteRule with frontend-extended RewriteTarget and _uid field. */
export interface UrlRewriteRule extends Omit<GeneratedUrlRewriteRule, "targets"> {
  targets: RewriteTarget[];
  /** Frontend-only unique ID for stable v-for keys. Not serialized by backend. */
  _uid?: number;
}

/** UrlRewriteSettings using extended UrlRewriteRule. */
export interface UrlRewriteSettings extends Omit<GeneratedUrlRewriteSettings, "rules"> {
  enabled: boolean;
  rules: UrlRewriteRule[];
}

/**
 * AppSettings with frontend-extended sub-types.
 * Generated counterpart is in generated/types.ts.
 */
export type AppSettings = Omit<
  GeneratedAppSettings,
  "appearance" | "ioBaseline" | "urlRewrite" | "aria2Rpc" | "cdnAcceleration"
> & {
  appearance: AppearanceSettings;
  ioBaseline: IoBaselineSettings;
  urlRewrite: UrlRewriteSettings;
  aria2Rpc: Aria2RpcSettings;
  cdnAcceleration: CdnAccelerationSettings;
};
