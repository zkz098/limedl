//! Settings field labels and validation error messages.

use super::{Language, tr, tr_fmt};

/// Settings field referenced by a validation error message.
///
/// The label is localized so an English UI never surfaces Chinese text from
/// Rust-side validation (Slint `@tr` only covers strings inside `.slint` files).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    MaxRetries,
    MaxParallelTasks,
    GlobalSpeedLimit,
    SchedulerMaxParallelThreads,
    SchedulerMaxThreadsPerTask,
    SchedulerMinThreadsPerTask,
    ListenPort,
    MaxPeersPerTorrent,
    BtMaxDownloads,
    BtMaxSeeds,
    BtMaxTorrents,
    BtActiveLimit,
    BtGlobalDownloadRateLimit,
    BtGlobalUploadRateLimit,
    BtUploadLimit,
    BtUploadRatioLimit,
    BtAntiLeechGraceSecs,
    BtAntiLeechRatio,
    BtAntiLeechBanSecs,
    BtAntiLeechMaxUploadSlots,
    BtMaxUploadSlotsPerTorrent,
    BtSmartBanMaxFailures,
    BtEvictionBanDurationSecs,
    BtDataContributionTimeoutSecs,
    IoBufferLimitMb,
    IoGameModeBufferMb,
    IoMaxParallelHdd,
    IoGameModeMaxParallel,
    IoSsdWriteCombineMb,
    LoggingRetentionCount,
    LoggingRetentionDays,
    Aria2Port,
    MaxInMemoryDownloads,
    SpeedLimitStartHour,
    SpeedLimitEndHour,
    SpeedLimitLimit,
    DiskOverridePath,
    SchedulerTickMs,
    SchedulerMaxConnectionsPerHost,
    SchedulerAimdDegradeThreshold,
    SchedulerAimdIncreaseThreshold,
    SchedulerAimdSevereDropThreshold,
    SchedulerAimdCooldownSecs,
    SchedulerAimdSamplesNeeded,
    SchedulerAimdProbeStableCycles,
    SchedulerAimdDecreaseFactor,
    SchedulerAimdStepRatio,
    SchedulerAimdOscillationThreshold,
    SchedulerAimdHysteresisMultiplier,
    SchedulerCustomChunkSizeMb,
    SchedulerTailSprintStallWindowSecs,
    SchedulerTailSprintMinSplitSizeMb,
    SchedulerWorkStealMinSplitSizeMb,
    SchedulerConnectTimeoutSecs,
    SchedulerReadTimeoutSecs,
    SchedulerPoolMaxIdlePerHost,
    SchedulerTcpKeepaliveSecs,
    SchedulerRetryBackoffBaseMs,
    SchedulerRetryBackoffMaxMs,
    SchedulerRetryBackoffJitterPercent,
}

impl SettingsField {
    /// Localized field label used as the subject of a validation message.
    pub fn label(self, lang: Language) -> &'static str {
        tr(self.msgid(), lang)
    }

    /// English source string of this field's label, and its catalog key.
    fn msgid(self) -> &'static str {
        use SettingsField as F;
        match self {
            F::MaxRetries => "Max retries",
            F::MaxParallelTasks => "Max parallel tasks",
            F::GlobalSpeedLimit => "Global speed limit",
            F::SchedulerMaxParallelThreads => "Scheduler max parallel threads",
            F::SchedulerMaxThreadsPerTask => "Max threads per task",
            F::SchedulerMinThreadsPerTask => "Min threads per task",
            F::ListenPort => "BT listen port",
            F::MaxPeersPerTorrent => "Max peers per torrent",
            F::BtMaxDownloads => "BT max downloads",
            F::BtMaxSeeds => "BT max seeds",
            F::BtMaxTorrents => "BT max torrents",
            F::BtActiveLimit => "BT active limit",
            F::BtGlobalDownloadRateLimit => "BT global download limit",
            F::BtGlobalUploadRateLimit => "BT global upload limit",
            F::BtUploadLimit => "Seeding upload limit",
            F::BtUploadRatioLimit => "Share ratio limit",
            F::BtAntiLeechGraceSecs => "Anti-leech grace period",
            F::BtAntiLeechRatio => "Anti-leech ratio threshold",
            F::BtAntiLeechBanSecs => "Anti-leech ban duration",
            F::BtAntiLeechMaxUploadSlots => "Anti-leech upload slots",
            F::BtMaxUploadSlotsPerTorrent => "Max upload slots per torrent",
            F::BtSmartBanMaxFailures => "Smart ban threshold",
            F::BtEvictionBanDurationSecs => "Eviction ban duration",
            F::BtDataContributionTimeoutSecs => "No-contribution timeout",
            F::IoBufferLimitMb => "IO buffer limit",
            F::IoGameModeBufferMb => "Game mode buffer",
            F::IoMaxParallelHdd => "Max parallel HDD transfers",
            F::IoGameModeMaxParallel => "Game mode max parallel",
            F::IoSsdWriteCombineMb => "SSD write-combine buffer",
            F::LoggingRetentionCount => "Log retention count",
            F::LoggingRetentionDays => "Log retention days",
            F::Aria2Port => "Aria2 port",
            F::MaxInMemoryDownloads => "In-memory record limit",
            F::SpeedLimitStartHour => "Speed limit schedule start hour",
            F::SpeedLimitEndHour => "Speed limit schedule end hour",
            F::SpeedLimitLimit => "Speed limit schedule rate",
            F::DiskOverridePath => "Media override directory",
            F::SchedulerTickMs => "Scheduler tick duration",
            F::SchedulerMaxConnectionsPerHost => "Max connections per host",
            F::SchedulerAimdDegradeThreshold => "AIMD degrade threshold",
            F::SchedulerAimdIncreaseThreshold => "AIMD increase threshold",
            F::SchedulerAimdSevereDropThreshold => "AIMD severe drop threshold",
            F::SchedulerAimdCooldownSecs => "AIMD cooldown duration",
            F::SchedulerAimdSamplesNeeded => "AIMD samples needed",
            F::SchedulerAimdProbeStableCycles => "AIMD probe stable cycles",
            F::SchedulerAimdDecreaseFactor => "AIMD decrease factor",
            F::SchedulerAimdStepRatio => "AIMD additive step ratio",
            F::SchedulerAimdOscillationThreshold => "AIMD oscillation threshold",
            F::SchedulerAimdHysteresisMultiplier => "AIMD hysteresis multiplier",
            F::SchedulerCustomChunkSizeMb => "Custom chunk size",
            F::SchedulerTailSprintStallWindowSecs => "Tail sprint stall window",
            F::SchedulerTailSprintMinSplitSizeMb => "Tail sprint min split size",
            F::SchedulerWorkStealMinSplitSizeMb => "Work steal min split size",
            F::SchedulerConnectTimeoutSecs => "Network connect timeout",
            F::SchedulerReadTimeoutSecs => "Network read timeout",
            F::SchedulerPoolMaxIdlePerHost => "Connection pool max idle per host",
            F::SchedulerTcpKeepaliveSecs => "TCP keepalive duration",
            F::SchedulerRetryBackoffBaseMs => "Retry backoff base delay",
            F::SchedulerRetryBackoffMaxMs => "Retry backoff max delay",
            F::SchedulerRetryBackoffJitterPercent => "Retry backoff jitter percent",
        }
    }
}

/// "expected a number" validation error for `field`.
pub fn format_validation_number(lang: Language, field: SettingsField, value: &str) -> String {
    tr_fmt(
        "Invalid {}: '{}' — please enter a valid number",
        lang,
        &[&field.label(lang), &value],
    )
}

/// "expected an integer" validation error for `field`.
pub fn format_validation_integer(lang: Language, field: SettingsField, value: &str) -> String {
    tr_fmt(
        "Invalid {}: '{}' — please enter a valid integer",
        lang,
        &[&field.label(lang), &value],
    )
}

/// "expected a TCP port" validation error for `field`.
pub fn format_validation_port(lang: Language, field: SettingsField, value: &str) -> String {
    tr_fmt(
        "Invalid {}: '{}' — please enter a port in 0-65535",
        lang,
        &[&field.label(lang), &value],
    )
}

/// "value out of range" validation error for `field`.
pub fn format_validation_range(lang: Language, field: SettingsField, min: u32, max: u32) -> String {
    tr_fmt(
        "{} must be between {} and {}",
        lang,
        &[&field.label(lang), &min, &max],
    )
}

/// Port 0 is rejected because it would let the OS pick an unpredictable port.
pub fn format_validation_port_zero(lang: Language, field: SettingsField) -> String {
    tr_fmt("{} cannot be 0", lang, &[&field.label(lang)])
}

/// Manual proxy mode requires a proxy URL.
pub fn format_proxy_url_required(lang: Language) -> String {
    tr(
        "A proxy URL is required when the proxy mode is manual",
        lang,
    )
    .to_string()
}

/// "expected an absolute path" validation error for `field`.
///
/// A relative key can never match a download destination — the engine's lookup
/// compares normalized absolute paths — and Windows detection silently answers
/// "SSD" for one, so the editor must not persist it.
pub fn format_validation_absolute_path(
    lang: Language,
    field: SettingsField,
    value: &str,
) -> String {
    tr_fmt(
        "{} must be an absolute path: '{}' (e.g. D:\\Downloads or \\\\NAS\\share)",
        lang,
        &[&field.label(lang), &value],
    )
}

/// Two rows of a list editor point at the same path.
pub fn format_validation_duplicate_path(lang: Language, value: &str) -> String {
    tr_fmt(
        "Duplicate path: '{}' — one entry per directory",
        lang,
        &[&value],
    )
}

/// Invalid IP literal typed into the CDN manual override field.
pub fn format_invalid_ip(lang: Language) -> &'static str {
    tr("Invalid IP address format", lang)
}

/// A per-client Aria2 token row was left without a name.
pub fn format_validation_client_name_empty(lang: Language) -> String {
    tr("Client name cannot be empty", lang).to_string()
}

/// Two per-client Aria2 rows share a name.
pub fn format_validation_client_name_duplicate(lang: Language, name: &str) -> String {
    tr_fmt("Duplicate client name: '{}'", lang, &[&name])
}

/// A per-client Aria2 row has no token (its generation failed or it was
/// hand-edited).
pub fn format_validation_client_token_missing(lang: Language, name: &str) -> String {
    tr_fmt("Client '{}' has no token — regenerate it", lang, &[&name])
}

/// Default label for a freshly added Aria2 client row.
pub fn format_aria2_client_default_name(index: usize, lang: Language) -> String {
    tr_fmt("Client {}", lang, &[&index])
}
