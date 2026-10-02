//! The self-update row in Settings → About.
//!
//! `handlers/updater.rs` owns the flows, but the phase → control matrix lives in
//! `.slint`: which button is offered (and with which label), what the status line
//! says and which install channel is named. That mapping is what tells a user to
//! restart, so a renamed phase string or a swapped `install_kind` would strand
//! them with no button to press — and nothing else in the suite reads it.
//!
//! Two rules for this file:
//!
//! * **Never click `download_btn` / `restart_btn`**: the first runs a real GitHub
//!   download, the second calls `update::restart_application()` and would replace
//!   the running test process. Presence, label and enabled state are the oracle.
//! * The assertions read the English `@tr` strings, so the window is built with
//!   `with_language(Language::EnUs)`. Every new window re-applies its own
//!   language, so nothing leaks into the sibling tests.

use crate::i18n::Language;

use super::{TestUi, with_language};

/// Put the window into one update phase, exactly like the updater handlers do.
fn set_phase(ui: &TestUi, phase: &str) {
    crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
        state.phase = phase.into();
    });
}

/// The text an element announces (a `Text`'s `accessible-label` is its `text`,
/// and the custom buttons declare theirs).
fn text(ui: &TestUi, id: &str) -> String {
    ui.find(id)
        .accessible_label()
        .unwrap_or_else(|| panic!("{id} does not announce a label"))
        .to_string()
}

#[test]
fn the_update_row_follows_the_phase_and_the_install_kind() {
    with_language(Language::EnUs, |ui| {
        // The About tab is a scrolling tab and the update row sits below the fold
        // at the fixture's default 800x600 window (the dialog is capped at
        // 820px, so a taller window is the way to give it room without
        // scrolling).
        ui.set_window_size(1100.0, 1600.0);
        ui.click("MainWindow::ta_set");
        ui.window.invoke_set_settings_tab(8);
        assert!(
            ui.has("SettingsTabAbout::check_btn"),
            "the About tab must be showing; visible ids: {:?}",
            ui.ids()
        );

        // `ui_boot` starts at "idle": nothing to download, nothing to restart.
        assert_eq!(
            text(ui, "SettingsTabAbout::update_status_text"),
            "No update checked yet."
        );
        assert_eq!(text(ui, "SettingsTabAbout::install_kind_text"), "Portable");
        assert_eq!(text(ui, "SettingsTabAbout::check_btn"), "Check for Updates");
        assert_eq!(
            ui.find("SettingsTabAbout::check_btn").accessible_enabled(),
            Some(true)
        );
        assert!(!ui.has("SettingsTabAbout::download_btn"));
        assert!(!ui.has("SettingsTabAbout::restart_btn"));

        set_phase(ui, "checking");
        assert_eq!(
            text(ui, "SettingsTabAbout::update_status_text"),
            "Checking for updates..."
        );
        assert_eq!(text(ui, "SettingsTabAbout::check_btn"), "Checking...");
        assert_eq!(
            ui.find("SettingsTabAbout::check_btn").accessible_enabled(),
            Some(false),
            "a second check must not stack on the first"
        );

        set_phase(ui, "up-to-date");
        assert_eq!(
            text(ui, "SettingsTabAbout::update_status_text"),
            "You are up to date."
        );
        assert_eq!(text(ui, "SettingsTabAbout::check_btn"), "Check for Updates");
        assert!(!ui.has("SettingsTabAbout::download_btn"));
        assert!(!ui.has("SettingsTabAbout::restart_btn"));

        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.phase = "available".into();
            state.latest_version = "9.9.9".into();
        });
        assert_eq!(
            text(ui, "SettingsTabAbout::update_status_text"),
            "Version 9.9.9 is available.",
            "the version the manifest reported has to reach the status line"
        );
        assert_eq!(
            text(ui, "SettingsTabAbout::download_btn"),
            "Download & Install"
        );
        assert!(
            ui.has("SettingsTabAbout::check_btn"),
            "re-checking stays available while an update is pending"
        );
        assert!(!ui.has("SettingsTabAbout::restart_btn"));

        // The install channel changes both the label and (in the handler) which
        // flow the button starts: a Store install hands the update to the OS
        // instead of downloading the portable artifact.
        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.install_kind = "store".into();
        });
        assert_eq!(
            text(ui, "SettingsTabAbout::install_kind_text"),
            "Store (MSIX)"
        );
        assert_eq!(
            text(ui, "SettingsTabAbout::download_btn"),
            "Update in Store"
        );

        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.install_kind = "installer".into();
        });
        assert_eq!(text(ui, "SettingsTabAbout::install_kind_text"), "Installer");
        assert_eq!(
            text(ui, "SettingsTabAbout::download_btn"),
            "Download & Install"
        );

        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.phase = "downloading".into();
            state.progress_percent = 42.0;
            state.progress_label = "1.0 / 2.0 MB".into();
        });
        assert!(ui.has("SettingsTabAbout::update_progress_row"));
        assert_eq!(
            text(ui, "SettingsTabAbout::update_progress_label"),
            "1.0 / 2.0 MB"
        );
        assert!(
            !ui.has("SettingsTabAbout::check_btn")
                && !ui.has("SettingsTabAbout::download_btn")
                && !ui.has("SettingsTabAbout::restart_btn"),
            "there is nothing to click while the download is running"
        );

        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.phase = "ready".into();
        });
        assert_eq!(
            text(ui, "SettingsTabAbout::restart_btn"),
            "Restart to Update",
            "a verified download is only useful if the restart button shows up"
        );
        assert!(!ui.has("SettingsTabAbout::update_progress_row"));
        assert!(!ui.has("SettingsTabAbout::check_btn"));

        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.phase = "error".into();
            state.error_text = "signature mismatch".into();
        });
        assert_eq!(
            text(ui, "SettingsTabAbout::update_status_text"),
            "Update check failed."
        );
        assert_eq!(
            text(ui, "SettingsTabAbout::update_error_text"),
            "signature mismatch",
            "the reason has to be readable, not just a generic failure line"
        );
        assert!(
            ui.has("SettingsTabAbout::check_btn"),
            "retrying stays possible"
        );

        // An error without a message must not leave an empty row behind.
        crate::ui_sync::push_update_state(&ui.ctx.ui_weak, |state| {
            state.error_text = "".into();
        });
        assert!(!ui.has("SettingsTabAbout::update_error_text"));
    });
}
