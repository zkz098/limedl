//! In-app toast text for every feature area.

use super::{Language, format_priority_label, tr, tr_fmt};

pub fn format_toast_task_added(file_name: &str, lang: Language) -> String {
    tr_fmt("Download task added: {}", lang, &[&file_name])
}

pub fn format_toast_task_add_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to add task: {}", lang, &[&err])
}

pub fn format_toast_batch_done(ok: usize, total: usize, lang: Language) -> String {
    tr_fmt("Batch add finished: {}/{} succeeded", lang, &[&ok, &total])
}

pub fn format_toast_settings_saved(lang: Language) -> &'static str {
    tr("Settings saved", lang)
}

pub fn format_toast_settings_save_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to save settings: {}", lang, &[&err])
}

pub fn format_toast_settings_invalid(err: &str, lang: Language) -> String {
    tr_fmt("Invalid settings: {}", lang, &[&err])
}

pub fn format_toast_setup_finished(lang: Language) -> &'static str {
    tr("Setup completed", lang)
}

pub fn format_toast_tracker_synced(count: usize, lang: Language) -> String {
    tr_fmt("Tracker list synced: {} entries", lang, &[&count])
}

pub fn format_toast_tracker_sync_failed(err: &str, lang: Language) -> String {
    tr_fmt("Tracker sync failed: {}", lang, &[&err])
}

pub fn format_toast_link_copied(lang: Language) -> &'static str {
    tr("Download link copied to clipboard", lang)
}

pub fn format_toast_filename_copied(lang: Language) -> &'static str {
    tr("File name copied to clipboard", lang)
}

pub fn format_toast_labs_saved(lang: Language) -> &'static str {
    tr("Labs settings saved", lang)
}

pub fn format_toast_labs_save_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to save Labs settings: {}", lang, &[&err])
}

pub fn format_toast_cdn_applied(ip: &str, lang: Language) -> String {
    tr_fmt("CDN node applied: {}", lang, &[&ip])
}

pub fn format_toast_cdn_cleared(lang: Language) -> &'static str {
    tr("CDN acceleration cleared", lang)
}

pub fn format_toast_cdn_test_done(ip: Option<&str>, lang: Language) -> String {
    match ip {
        Some(ip) => tr_fmt("CDN speedtest finished, node {} locked", lang, &[&ip]),
        None => tr("CDN speedtest finished", lang).to_string(),
    }
}

pub fn format_toast_cdn_test_failed(err: &str, lang: Language) -> String {
    tr_fmt("CDN speedtest failed: {}", lang, &[&err])
}

pub fn format_toast_aria2_rpc_started(port: u16, lang: Language) -> String {
    tr_fmt("Aria2 RPC started (port {})", lang, &[&port])
}

pub fn format_toast_aria2_rpc_stopped(lang: Language) -> &'static str {
    tr("Aria2 RPC stopped", lang)
}

pub fn format_toast_aria2_token_copied(lang: Language) -> &'static str {
    tr("Token copied", lang)
}

pub fn format_toast_aria2_token_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to generate token: {}", lang, &[&err])
}

pub fn format_toast_autostart_failed(err: &str, lang: Language) -> String {
    tr_fmt("Autostart failed: {}", lang, &[&err])
}
/// Toast confirming a priority change (used by the priority popup menu).
pub fn format_toast_priority_set(
    file_name: &str,
    priority: limedl_core::types::Priority,
    lang: Language,
) -> String {
    let label = format_priority_label(priority, lang);
    tr_fmt("Priority set to {} — {}", lang, &[&label, &file_name])
}

/// Toast shown when changing the priority failed.
pub fn format_toast_priority_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to set priority: {}", lang, &[&err])
}

/// Toast shown when the user tries to deselect every BT file.
pub fn format_toast_bt_files_keep_one(lang: Language) -> &'static str {
    tr("At least one file must stay selected", lang)
}

/// Toast shown when updating the BT file selection failed.
pub fn format_toast_bt_files_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to update torrent file selection: {}", lang, &[&err])
}

/// Toast shown after the "clear completed" action removed `count` records.
pub fn format_toast_clear_completed(count: usize, lang: Language) -> String {
    tr_fmt("Cleared {} completed record(s)", lang, &[&count])
}

/// Toast shown when "clear completed" found nothing to remove.
pub fn format_toast_clear_completed_none(lang: Language) -> &'static str {
    tr("No completed records to clear", lang)
}

/// Toast shown when enumerating the task list failed.
pub fn format_toast_clear_completed_failed(lang: Language) -> &'static str {
    tr("Failed to clear completed records", lang)
}

/// Toast shown after a successful factory reset (the app then restarts).
pub fn format_toast_factory_reset_done(lang: Language) -> &'static str {
    tr("Factory reset complete, restarting…", lang)
}

/// Toast shown when the factory reset could not delete the data directory.
pub fn format_toast_factory_reset_failed(err: &str, lang: Language) -> String {
    tr_fmt("Factory reset failed: {}", lang, &[&err])
}

pub fn format_toast_update_downloading(version: &str, lang: Language) -> String {
    tr_fmt("Downloading update v{}...", lang, &[&version])
}

pub fn format_toast_update_ready(version: &str, lang: Language) -> String {
    tr_fmt(
        "v{} downloaded and verified. Restart to apply.",
        lang,
        &[&version],
    )
}

pub fn format_toast_update_installer_launched(lang: Language) -> &'static str {
    tr("Installer launched. Exiting to complete update.", lang)
}

pub fn format_toast_update_failed(err: &str, lang: Language) -> String {
    tr_fmt("Update failed: {}", lang, &[&err])
}

pub fn format_toast_update_store_triggered(lang: Language) -> &'static str {
    tr("Opening Microsoft Store to update...", lang)
}

pub fn format_toast_update_not_found(lang: Language) -> &'static str {
    tr(
        "No update information found. Please check for updates again.",
        lang,
    )
}

pub fn format_toast_update_up_to_date(lang: Language) -> &'static str {
    tr("You are already on the latest version.", lang)
}

pub fn format_toast_update_available(version: &str, lang: Language) -> String {
    tr_fmt(
        "New version v{} available. Go to About to update.",
        lang,
        &[&version],
    )
}

pub fn format_toast_update_check_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to check for updates: {}", lang, &[&err])
}

pub fn format_toast_update_restart_failed(err: &str, lang: Language) -> String {
    tr_fmt("Failed to restart: {}", lang, &[&err])
}

pub fn format_toast_invalid_checksum(lang: Language) -> String {
    tr(
        "Invalid checksum format (supports SHA-256, SHA-512, BLAKE3)",
        lang,
    )
    .to_string()
}

pub fn format_toast_integrity_checking(file_name: &str, lang: Language) -> String {
    tr_fmt("Verifying integrity: {}", lang, &[&file_name])
}

pub fn format_toast_integrity_passed(file_name: &str, mode: &str, lang: Language) -> String {
    tr_fmt(
        "{} verified successfully ({} match)",
        lang,
        &[&file_name, &mode],
    )
}

pub fn format_toast_integrity_mismatch(file_name: &str, mode: &str, lang: Language) -> String {
    tr_fmt(
        "{} verification failed: {} mismatch!",
        lang,
        &[&file_name, &mode],
    )
}

pub fn format_toast_integrity_computed(file_name: &str, mode: &str, lang: Language) -> String {
    tr_fmt(
        "{} integrity computed ({}), hash copied to clipboard",
        lang,
        &[&file_name, &mode],
    )
}

pub fn format_toast_integrity_failed(err: &str, lang: Language) -> String {
    tr_fmt("Integrity verification failed: {}", lang, &[&err])
}
