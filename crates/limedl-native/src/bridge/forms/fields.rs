//! Single source of truth for the `SettingsFormData` fields.
//!
//! Every field of the Slint-generated `SettingsFormData` appears exactly once in
//! the [`settings_form_fields!`] table below. The table is what keeps the three
//! hand-written settings paths (`to_form.rs`, `from_form.rs` and
//! `aria2_rpc::options`) from silently drifting apart when a field is added:
//!
//! * [`FormFields::validate`] derives the numeric validation of the text fields
//!   (parse type + localized message function) from the `uNN`/`f64` rows, so the
//!   five near-identical `check_*` closures that used to live in `from_form.rs`
//!   no longer have to be kept in sync by hand.
//! * The test-only helpers ([`FormFields::sentinel_form`],
//!   [`FormFields::changed`], [`FormFields::names`]) drive the completeness test
//!   in this module, which fails when a field is dropped from either direction of
//!   the conversion or when a new `AppSettings` leaf is not wired to the form.
//!
//! Rows are tagged by kind:
//!
//! * `str` — text field, no validation;
//! * `bool` — checkbox;
//! * `idx` — ComboBox index into one of the `combo.rs` lists;
//! * `u16` / `u32` / `usize` / `u64` / `f64` — numeric text field; the parenthesised
//!   identifier is the [`crate::i18n::SettingsField`] label used in the message;
//! * `display` — a field that is *not* part of the form↔settings round-trip: it
//!   is a runtime display value (`game_mode`, `app_version`, …), a UI-only
//!   convenience (the expert-preset picker), or a value the conversion derives
//!   instead of reading (the always-on `file` column). It is still listed here so
//!   the field-name completeness test covers it.
//!
//! The sentinel is the value the completeness test writes into the form. For
//! `bool`/`idx` rows it is `flip`, meaning the test writes the opposite of the
//! rendered default so it does not have to know each field's default; the other
//! kinds carry a bare string literal (wrapped into a `SharedString` by the
//! macro) chosen to survive the clamps in `from_form.rs` and to differ from the
//! default.

#[cfg(test)]
use slint::SharedString;

use crate::SettingsFormData;
use crate::i18n::{self, Language};

/// Parse a trimmed text field, skipping empty input.
///
/// `message` is the localized formatter for the field's numeric class, so the
/// error text stays identical to the hand-written version this replaced.
fn check_parse<T: std::str::FromStr>(
    raw: &str,
    lang: Language,
    field: i18n::SettingsField,
    message: fn(Language, i18n::SettingsField, &str) -> String,
) -> Result<(), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    trimmed
        .parse::<T>()
        .map(|_| ())
        .map_err(|_| message(lang, field, trimmed))
}

/// The other end of a ComboBox list, so a sentinel index is always a valid one
/// and always differs from the default.
#[cfg(test)]
fn flip_combo_index(index: i32) -> i32 {
    if index == 0 { 1 } else { 0 }
}

/// Declares the field table once and generates the accessors built on it.
macro_rules! settings_form_fields {
    ($( $kind:ident $field:ident $(($label:ident))? = $sentinel:tt ; )+) => {
        /// Field-table accessors; see the module documentation.
        pub(crate) struct FormFields;

        impl FormFields {
            /// Validate every numeric text field, skipping empty input.
            ///
            /// Replaces the per-field `check_*(...)` calls that used to sit in
            /// `from_form::validate_form`; the kinds and messages are unchanged.
            pub(crate) fn validate(
                form: &SettingsFormData,
                lang: Language,
            ) -> Result<(), String> {
                $(
                    settings_form_fields!(@validate form, lang, $kind $field $(($label))?);
                )+
                Ok(())
            }

            /// Every field name, in table order.
            #[cfg(test)]
            pub(crate) fn names() -> &'static [&'static str] {
                &[ $( stringify!($field), )+ ]
            }

            /// A form whose round-trip fields all hold their sentinel value.
            /// The `display` rows are left at `Default::default()`.
            #[cfg(test)]
            pub(crate) fn sentinel_form(base: &SettingsFormData) -> SettingsFormData {
                let mut form = SettingsFormData::default();
                $(
                    settings_form_fields!(@sent form, base, $kind $field, $sentinel);
                )+
                form
            }

            /// Names of the round-trip fields whose value differs between the
            /// two forms.
            #[cfg(test)]
            pub(crate) fn changed(
                base: &SettingsFormData,
                other: &SettingsFormData,
            ) -> Vec<&'static str> {
                let mut changed = Vec::new();
                $(
                    settings_form_fields!(@diff changed, base, other, $kind $field);
                )+
                changed
            }
        }
    };

    // ── @validate: one arm per numeric kind; everything else validates nothing ──
    (@validate $form:ident, $lang:ident, u16 $field:ident ($label:ident)) => {
        check_parse::<u16>(
            $form.$field.trim(),
            $lang,
            i18n::SettingsField::$label,
            i18n::format_validation_port,
        )?;
    };
    (@validate $form:ident, $lang:ident, u32 $field:ident ($label:ident)) => {
        check_parse::<u32>(
            $form.$field.trim(),
            $lang,
            i18n::SettingsField::$label,
            i18n::format_validation_integer,
        )?;
    };
    (@validate $form:ident, $lang:ident, usize $field:ident ($label:ident)) => {
        check_parse::<usize>(
            $form.$field.trim(),
            $lang,
            i18n::SettingsField::$label,
            i18n::format_validation_integer,
        )?;
    };
    (@validate $form:ident, $lang:ident, u64 $field:ident ($label:ident)) => {
        check_parse::<u64>(
            $form.$field.trim(),
            $lang,
            i18n::SettingsField::$label,
            i18n::format_validation_number,
        )?;
    };
    (@validate $form:ident, $lang:ident, f64 $field:ident ($label:ident)) => {
        check_parse::<f64>(
            $form.$field.trim(),
            $lang,
            i18n::SettingsField::$label,
            i18n::format_validation_number,
        )?;
    };
    (@validate $form:ident, $lang:ident, $kind:ident $field:ident) => {};

    // ── @sent: write the sentinel, skipping runtime/derived rows ──
    // `bool` and `idx` rows say `flip`: the value is the opposite of the
    // rendered default, computed here because the table cannot name the base
    // form (macro hygiene). The remaining kinds carry a bare string literal
    // that is wrapped into a `SharedString` here.
    (@sent $form:ident, $base:ident, display $field:ident, $sentinel:tt) => {};
    (@sent $form:ident, $base:ident, bool $field:ident, flip) => {
        $form.$field = !$base.$field;
    };
    (@sent $form:ident, $base:ident, idx $field:ident, flip) => {
        $form.$field = flip_combo_index($base.$field);
    };
    (@sent $form:ident, $base:ident, $kind:ident $field:ident, $sentinel:tt) => {
        $form.$field = SharedString::from($sentinel);
    };

    // ── @diff: compare the round-tripped fields ──
    (@diff $changed:ident, $base:ident, $other:ident, display $field:ident) => {};
    (@diff $changed:ident, $base:ident, $other:ident, $kind:ident $field:ident) => {
        if $base.$field != $other.$field {
            $changed.push(stringify!($field));
        }
    };
}

settings_form_fields! {
    // ── 常规下载 ──
    str   default_download_dir = "sentinel-download-dir";
    usize max_parallel_tasks (MaxParallelTasks) = "13";
    u64   global_speed_limit_kb (GlobalSpeedLimit) = "13";
    u32   download_max_retries (MaxRetries) = "13";
    idx   download_checksum_idx = flip;
    bool  download_auto_detect_sha256 = flip;
    str   download_user_agent = "sentinel-user-agent";

    // ── 外观 / 启动 ──
    idx   appearance_color_mode_idx = flip;
    idx   appearance_theme_color_idx = flip;
    idx   appearance_background_opacity_idx = flip;
    idx   appearance_language_idx = flip;
    idx   appearance_close_behavior_idx = flip;
    bool  appearance_show_detail_info = flip;
    bool  appearance_compact_view = flip;
    // The file column is forced on by `from_form` (an all-false selection would
    // hide the task name), so it cannot round-trip; `visible_columns` as a whole
    // is still covered by the other nine columns.
    display appearance_column_file = ();
    bool  appearance_column_size = flip;
    bool  appearance_column_downloaded = flip;
    bool  appearance_column_status = flip;
    bool  appearance_column_progress = flip;
    bool  appearance_column_speed = flip;
    bool  appearance_column_priority = flip;
    bool  appearance_column_upload_speed = flip;
    bool  appearance_column_seeds = flip;
    bool  appearance_column_eta = flip;
    bool  autostart = flip;
    bool  prevent_sleep = flip;
    bool  notifications_enabled = flip;
    idx   double_click_completed_idx = flip;
    idx   double_click_uncompleted_idx = flip;

    // ── 代理 ──
    idx   proxy_mode_idx = flip;
    str   proxy_manual_url = "http://127.0.0.1:7890";

    // ── 调度 ──
    idx   scheduler_mode_idx = flip;
    usize scheduler_max_parallel_threads (SchedulerMaxParallelThreads) = "13";
    usize scheduler_max_threads_per_task (SchedulerMaxThreadsPerTask) = "13";
    usize scheduler_min_threads_per_task (SchedulerMinThreadsPerTask) = "13";
    idx   scheduler_adaptive_profile_idx = flip;
    idx   scheduler_chunk_strategy_idx = flip;
    bool  scheduler_tail_sprint = flip;
    bool  scheduler_connection_warmup = flip;
    bool  scheduler_expert_mode = flip;
    // Derived from the expert values by `to_form` and written by the dialog's
    // preset buttons, never by `from_form`: a display-only convenience.
    display scheduler_expert_preset_idx = ();
    u64   scheduler_tick_ms (SchedulerTickMs) = "13";
    usize scheduler_max_connections_per_host (SchedulerMaxConnectionsPerHost) = "13";
    idx   scheduler_queue_sort_strategy_idx = flip;
    f64   scheduler_aimd_degrade_threshold (SchedulerAimdDegradeThreshold) = "0.31";
    f64   scheduler_aimd_increase_threshold (SchedulerAimdIncreaseThreshold) = "0.31";
    f64   scheduler_aimd_severe_drop_threshold (SchedulerAimdSevereDropThreshold) = "0.31";
    u64   scheduler_aimd_cooldown_secs (SchedulerAimdCooldownSecs) = "13";
    u32   scheduler_aimd_samples_needed (SchedulerAimdSamplesNeeded) = "13";
    u32   scheduler_aimd_probe_stable_cycles (SchedulerAimdProbeStableCycles) = "13";
    f64   scheduler_aimd_decrease_factor (SchedulerAimdDecreaseFactor) = "0.31";
    f64   scheduler_aimd_step_ratio (SchedulerAimdStepRatio) = "0.31";
    u32   scheduler_aimd_oscillation_threshold (SchedulerAimdOscillationThreshold) = "13";
    u32   scheduler_aimd_hysteresis_multiplier (SchedulerAimdHysteresisMultiplier) = "13";
    u64   scheduler_custom_chunk_size_mb (SchedulerCustomChunkSizeMb) = "13";
    u64   scheduler_tail_sprint_stall_window_secs (SchedulerTailSprintStallWindowSecs) = "13";
    u64   scheduler_tail_sprint_min_split_size_mb (SchedulerTailSprintMinSplitSizeMb) = "13";
    u64   scheduler_work_steal_min_split_size_mb (SchedulerWorkStealMinSplitSizeMb) = "13";
    u64   scheduler_connect_timeout_secs (SchedulerConnectTimeoutSecs) = "13";
    u64   scheduler_read_timeout_secs (SchedulerReadTimeoutSecs) = "13";
    usize scheduler_pool_max_idle_per_host (SchedulerPoolMaxIdlePerHost) = "13";
    u64   scheduler_tcp_keepalive_secs (SchedulerTcpKeepaliveSecs) = "13";
    u64   scheduler_retry_backoff_base_ms (SchedulerRetryBackoffBaseMs) = "13";
    u64   scheduler_retry_backoff_max_ms (SchedulerRetryBackoffMaxMs) = "13";
    u64   scheduler_retry_backoff_jitter_percent (SchedulerRetryBackoffJitterPercent) = "13";

    // ── BT 网络（基础 + 进阶） ──
    bool  bt_lightweight_mode = flip;
    bool  dht_enabled = flip;
    u16   listen_port (ListenPort) = "13";
    u32   max_bt_connections (MaxPeersPerTorrent) = "13";
    str   tracker_url = "http://tracker.invalid/announce";
    bool  bt_upnp_enabled = flip;
    bool  bt_natpmp_enabled = flip;
    bool  bt_ipv6_enabled = flip;
    bool  bt_pex_enabled = flip;
    bool  bt_lsd_enabled = flip;
    bool  bt_utp_enabled = flip;
    idx   bt_encryption_mode_idx = flip;
    idx   bt_preallocate_mode_idx = flip;
    u32   bt_max_downloads (BtMaxDownloads) = "13";
    u32   bt_max_seeds (BtMaxSeeds) = "13";
    u32   bt_max_torrents (BtMaxTorrents) = "13";
    u32   bt_active_limit (BtActiveLimit) = "13";
    u64   bt_global_download_rate_limit_kb (BtGlobalDownloadRateLimit) = "13";
    u64   bt_global_upload_rate_limit_kb (BtGlobalUploadRateLimit) = "13";
    bool  bt_enable_fast_extension = flip;
    bool  bt_enable_holepunch = flip;
    bool  bt_enable_web_seed = flip;
    bool  bt_enable_super_seeding = flip;

    // ── BT 做种与反吸血 ──
    bool  bt_pause_upload_when_limit = flip;
    u64   bt_upload_limit_kb (BtUploadLimit) = "13";
    f64   bt_upload_ratio_limit (BtUploadRatioLimit) = "13";
    bool  bt_anti_leech_enabled = flip;
    idx   bt_anti_leech_action_idx = flip;
    u64   bt_anti_leech_grace_secs (BtAntiLeechGraceSecs) = "13";
    // Clamped to 0.0..=1.0 by `from_form`.
    f64   bt_anti_leech_ratio (BtAntiLeechRatio) = "0.9";
    u64   bt_anti_leech_ban_secs (BtAntiLeechBanSecs) = "13";
    u32   bt_anti_leech_max_upload_slots (BtAntiLeechMaxUploadSlots) = "13";
    bool  bt_blocklist_enabled = flip;
    str   bt_blocklist_path = "sentinel-blocklist";

    // ── BT 引擎调优 ──
    idx   bt_seed_choking_algorithm_idx = flip;
    idx   bt_choking_algorithm_idx = flip;
    u32   bt_max_upload_slots_per_torrent (BtMaxUploadSlotsPerTorrent) = "13";
    u32   bt_smart_ban_max_failures (BtSmartBanMaxFailures) = "13";
    bool  bt_smart_ban_parole = flip;
    u64   bt_eviction_ban_duration_secs (BtEvictionBanDurationSecs) = "13";
    u64   bt_data_contribution_timeout_secs (BtDataContributionTimeoutSecs) = "13";

    // ── IO 基线 ──
    u64   io_buffer_limit_mb (IoBufferLimitMb) = "2001";
    u64   io_game_mode_buffer_mb (IoGameModeBufferMb) = "2001";
    u32   io_max_parallel_hdd (IoMaxParallelHdd) = "13";
    u32   io_game_mode_max_parallel (IoGameModeMaxParallel) = "5";
    bool  io_hdd_buffer_enabled = flip;
    u64   io_ssd_write_combine_mb (IoSsdWriteCombineMb) = "2001";

    // ── 日志 ──
    bool  logging_enabled = flip;
    idx   logging_level_idx = flip;
    str   logging_file_path = "sentinel-log-path";
    u32   logging_retention_count (LoggingRetentionCount) = "13";
    u32   logging_retention_days (LoggingRetentionDays) = "13";

    // ── Aria2 RPC ──
    bool  aria2_enabled = flip;
    u16   aria2_port (Aria2Port) = "6801";
    str   aria2_secret = "sentinel-secret";
    idx   aria2_auth_mode_idx = flip;

    // ── 运行态只读展示（由 `to_form` 的入参填充，不落盘） ──
    display game_mode = ();
    display overclock_mode = ();
    display io_status_text = ();
    display disk_type_text = ();

    // ── 高级与关于 ──
    usize max_in_memory_downloads (MaxInMemoryDownloads) = "50";
    display app_version = ();
    display engine_version = ();
    display arch_info = ();
    display graphics_renderer = ();
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use limedl_core::types::AppSettings;

    use super::*;
    use crate::bridge::{app_settings_to_form, update_app_settings_from_form};

    /// Settings leaves that are intentionally not edited from the settings
    /// dialog, with the reason. Each entry is a deliberate statement, which is
    /// the point of the list: a new leaf that lands here silently is exactly
    /// what this test exists to prevent.
    const NOT_ON_THE_FORM: &[&str] = &[
        // Written by the setup wizard, not the settings dialog.
        "setupCompleted",
        "lastSetupStep",
        // Written by the table header (see `ui_sync::apply_sort`), which
        // persists the sort order so it survives a restart.
        "appearance.sortKey",
        "appearance.sortDirection",
        // Engine-only knobs: they have no dialog control in the desktop client
        // and are set through the settings file / the server CLI.
        "appearance.showHeatmap",
        "scheduler.http3AltSvcEnabled",
        "aria2Rpc.listenAddress",
        "aria2Rpc.corsAllowedOrigins",
        "aria2Rpc.allowAnyOrigin",
        "aria2Rpc.exitOnShutdown",
        // Engine-managed state, not a user preference.
        "bt.trackerList",
        // Edited by their own dialogs: the Labs dialog (CDN + URL rewrite),
        // the Aria2 client list and the speed-limit schedule.
        "speedLimitSchedule",
        "urlRewrite.enabled",
        "urlRewrite.rules",
        "aria2Rpc.clients",
        "cdnAcceleration.enabled",
        "cdnAcceleration.provider",
        "cdnAcceleration.customTestUrl",
        "cdnAcceleration.customCidrs",
        "cdnAcceleration.activeIp",
        "cdnAcceleration.activeSpeedMbps",
        "cdnAcceleration.lastTestAtMs",
        "cdnAcceleration.lastError",
    ];

    /// Field names declared by `export struct SettingsFormData` in
    /// `ui/types.slint`, i.e. the contract the conversions must satisfy.
    fn declared_form_fields() -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/types.slint");
        let source = std::fs::read_to_string(&path).expect("read ui/types.slint");
        let start = source
            .find("export struct SettingsFormData")
            .expect("SettingsFormData declaration in ui/types.slint");
        let after_brace = &source[start..];
        let body = &after_brace[after_brace.find('{').expect("struct body") + 1..];
        let body = &body[..body.find("\n}").expect("struct end")];

        body.lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with("//") {
                    return None;
                }
                Some(line.split_once(':')?.0.trim().to_string())
            })
            .collect()
    }

    /// Flatten a settings value into `dotted.path -> rendered value`, treating
    /// arrays as a single leaf (the form edits them as one row list).
    fn json_leaves(value: &serde_json::Value) -> BTreeMap<String, String> {
        fn walk(value: &serde_json::Value, prefix: &str, out: &mut BTreeMap<String, String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, child) in map {
                        let path = if prefix.is_empty() {
                            key.clone()
                        } else {
                            format!("{prefix}.{key}")
                        };
                        walk(child, &path, out);
                    }
                }
                other => {
                    out.insert(prefix.to_string(), other.to_string());
                }
            }
        }

        let mut out = BTreeMap::new();
        walk(value, "", &mut out);
        out
    }

    /// Every field of `SettingsFormData` is declared in the table, and vice
    /// versa: a field added to `ui/types.slint` without a table row fails here.
    #[test]
    fn table_covers_every_settings_form_field() {
        let mut declared = declared_form_fields();
        declared.sort();

        let mut table: Vec<String> = FormFields::names().iter().map(|n| (*n).to_string()).collect();
        table.sort();

        let missing: Vec<&String> = declared.iter().filter(|f| !table.contains(f)).collect();
        let stale: Vec<&String> = table.iter().filter(|f| !declared.contains(f)).collect();
        assert!(
            missing.is_empty() && stale.is_empty(),
            "ui/types.slint and forms/fields.rs disagree:\n\
             missing from the table: {missing:?}\n\
             not in ui/types.slint: {stale:?}"
        );
    }

    /// The completeness test: a sentinel form is pushed through
    /// `from_form` and back through `to_form`, and every field and every
    /// `AppSettings` leaf has to move.
    ///
    /// Both assertions are difference-based rather than equality-based on
    /// purpose: the conversion clamps and reformats values (kib conversion,
    /// `{:.2}` floats, forced-on columns), so requiring exact round-trip
    /// equality would make the test a second copy of those rules. Requiring
    /// *movement* is enough to catch the bug class this guards: a field that is
    /// added to one side and forgotten on the other.
    #[test]
    fn every_settings_leaf_round_trips_through_the_form() {
        let lang = Language::EnUs;
        let base_settings = AppSettings::default();
        let base_form = app_settings_to_form(&base_settings, false, false, "", "", lang);
        let sentinel = FormFields::sentinel_form(&base_form);

        let mut applied = AppSettings::default();
        update_app_settings_from_form(&mut applied, &sentinel, lang)
            .expect("the sentinel form must pass validation");

        // Direction 1: `to_form` has to render exactly what `from_form` wrote.
        // Exact equality rather than "the field moved": the sentinels are chosen
        // to survive the clamps and the rounding, so a clamp that silently
        // swallows a value (or a `{:.2}`-style reformat that drifts) fails here
        // too. Only the `display` rows are excluded, and only `appearance_column_file`
        // is forced by either direction (`from_form` always keeps the file column).
        let rendered = app_settings_to_form(&applied, false, false, "", "", lang);
        let lossy = FormFields::changed(&sentinel, &rendered);
        assert!(
            lossy.is_empty(),
            "fields that do not round-trip exactly through `from_form` + `to_form`\n\
             (widen the sentinel, or the conversion is lossy): {lossy:#?}"
        );

        // Direction 2: `from_form` has to write every settings leaf.
        let base_leaves = json_leaves(&serde_json::to_value(&base_settings).expect("serialize"));
        let applied_leaves = json_leaves(&serde_json::to_value(&applied).expect("serialize"));
        let unchanged: Vec<&String> = base_leaves
            .iter()
            .filter(|(path, value)| {
                !NOT_ON_THE_FORM.contains(&path.as_str())
                    && applied_leaves.get(*path) == Some(value)
            })
            .map(|(path, _)| path)
            .collect();
        assert!(
            unchanged.is_empty(),
            "`AppSettings` leaves with no settings-dialog field, and not listed in\n\
             NOT_ON_THE_FORM: {unchanged:#?}"
        );
    }
}
