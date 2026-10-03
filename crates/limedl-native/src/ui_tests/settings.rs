//! Settings: the speed-limit schedule editor, the dialog's own state, and what
//! a rejected save leaves behind.
//!
//! `collect_settings` (and therefore the whole save path) lives behind a
//! `tokio::spawn`, so those two tests are `with_ui_async` + `pump_until`. The
//! schedule editor itself is fully synchronous and needs no pumping.

use crate::SpeedLimitSlotItem;
use crate::DiskTypeOverrideItem;
use crate::Aria2ClientItem;
use slint::Model;

use super::{TestUi, absolute_dir, with_ui};

fn slots(ui: &TestUi) -> Vec<SpeedLimitSlotItem> {
    let model = ui.window.get_speed_limit_slots();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .collect()
}

fn override_rows(ui: &TestUi) -> Vec<DiskTypeOverrideItem> {
    let model = ui.window.get_disk_type_overrides();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .collect()
}

fn client_rows(ui: &TestUi) -> Vec<Aria2ClientItem> {
    let model = ui.window.get_aria2_clients();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .collect()
}

/// The per-client token editor mints a token in the add/regenerate handler (the
/// only place Argon2 runs) and reveals the plaintext until Save. This pins that
/// the plaintext and its hash always move together, and that removal only drops
/// the row it was asked for.
#[test]
fn the_aria2_client_editor_generates_reveals_and_removes_rows() {
    with_ui(|ui| {
        ui.window.invoke_open_settings();
        assert!(client_rows(ui).is_empty());

        ui.window.invoke_aria2_client_add();
        let rows = client_rows(ui);
        assert_eq!(rows.len(), 1);
        assert!(
            !rows[0].name.is_empty(),
            "a new row gets a localized default name"
        );
        assert_eq!(rows[0].token.len(), 43, "the plaintext is revealed once");
        assert!(rows[0].token_hash.starts_with("$argon2id$"));
        assert!(!rows[0].created_text.is_empty());
        let first_token = rows[0].token.clone();
        let first_hash = rows[0].token_hash.clone();

        // Regenerate replaces both the plaintext and its hash.
        ui.window.invoke_aria2_client_regenerate(0);
        let rows = client_rows(ui);
        assert_ne!(rows[0].token, first_token);
        assert_ne!(rows[0].token_hash, first_hash);

        // A second row, then removing the first must keep the second's data.
        ui.window.invoke_aria2_client_add();
        ui.window
            .invoke_aria2_client_name_edited(1, "Phone".into());
        assert_eq!(client_rows(ui).len(), 2);
        assert_eq!(client_rows(ui)[1].name.as_str(), "Phone");

        ui.window.invoke_aria2_client_remove(0);
        let rows = client_rows(ui);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_str(), "Phone");
        assert!(
            !rows[0].token.is_empty(),
            "the surviving row keeps its revealed token"
        );

        // Out-of-range indices are no-ops, not panics.
        ui.window.invoke_aria2_client_remove(9);
        ui.window.invoke_aria2_client_regenerate(9);
        ui.window
            .invoke_aria2_client_name_edited(9, "nope".into());
        assert_eq!(client_rows(ui).len(), 1);
    });
}

/// The per-client name field has no per-row Save button: typing must land in
/// the model (so the dialog's Save persists it) and Enter must not be a no-op.
/// This clicks the real `CustomTextInput` instead of invoking the callback,
/// which is what pins the `edited`/`accepted` wiring in `tab_aria2.slint`.
#[test]
fn typing_and_enter_in_the_aria2_client_name_field_reach_the_model() {
    with_ui(|ui| {
        // The client row is the last section of the tab, so at the minimum
        // window size it starts below the ScrollView's fold and element queries
        // prune it. The app's preferred size keeps it on screen.
        ui.set_window_size(1280.0, 800.0);
        ui.window.invoke_open_settings();
        ui.window.invoke_set_settings_tab(7);
        let mut form = ui.window.get_settings_form();
        form.aria2_auth_mode_idx = 1;
        ui.window.set_settings_form(form);
        ui.window.invoke_aria2_client_add();

        ui.click("SettingsTabAria2::ta-client-name");
        ui.press_keys(&[slint::platform::Key::Control.into(), 'a']);
        ui.type_text("Phone");
        ui.press_keys(&[slint::platform::Key::Return.into()]);

        assert_eq!(client_rows(ui)[0].name.as_str(), "Phone");
    });
}

#[test]
fn the_schedule_editor_edits_rows_in_place_and_removes_only_the_one_asked_for() {
    with_ui(|ui| {
        ui.window.invoke_open_settings();

        ui.window.invoke_schedule_set_enabled(true);
        let rows = slots(ui);
        assert_eq!(rows.len(), 1, "turning the schedule on seeds one row");
        assert_eq!(rows[0].start_hour.as_str(), "0");
        assert_eq!(rows[0].end_hour.as_str(), "6");

        ui.window.invoke_schedule_add();
        assert_eq!(slots(ui).len(), 2);

        ui.window
            .invoke_schedule_update(0, "start".into(), "22".into());
        ui.window
            .invoke_schedule_update(0, "end".into(), "6".into());
        ui.window
            .invoke_schedule_update(0, "limit".into(), "512".into());

        let rows = slots(ui);
        assert_eq!(rows[0].start_hour.as_str(), "22");
        assert_eq!(rows[0].end_hour.as_str(), "6");
        assert_eq!(rows[0].limit_kb.as_str(), "512");
        assert!(rows[0].wraps, "22:00 → 06:00 crosses midnight");
        assert!(
            !rows[0].summary.is_empty(),
            "the row text is derived on edit"
        );
        assert_eq!(
            rows[1].start_hour.as_str(),
            "0",
            "the other row is untouched"
        );

        // Removing row 0 must not bring the *other* row's values along.
        ui.window.invoke_schedule_remove(0);
        let rows = slots(ui);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].start_hour.as_str(), "0");
        assert_eq!(rows[0].end_hour.as_str(), "6");

        // Out of range: no-op, no panic.
        ui.window.invoke_schedule_remove(9);
        assert_eq!(slots(ui).len(), 1);

        // Unknown field: the row keeps its text (the editor is the user's).
        ui.window
            .invoke_schedule_update(0, "nonsense".into(), "7".into());
        assert_eq!(slots(ui)[0].end_hour.as_str(), "6");

        ui.window.invoke_schedule_set_enabled(false);
        assert!(
            slots(ui).is_empty(),
            "turning the schedule off clears the rows"
        );
    });
}

#[test]
fn the_schedule_rows_keep_what_was_typed_and_clamp_only_the_derived_text() {
    with_ui(|ui| {
        ui.window.invoke_open_settings();
        ui.window.invoke_schedule_set_enabled(true);

        // The editor writes through whatever the user typed; the *derived*
        // summary and the midnight marker are what get clamped. Pinning this
        // asymmetry keeps "fix the summary" from silently rejecting input.
        ui.window
            .invoke_schedule_update(0, "start".into(), "99".into());
        ui.window
            .invoke_schedule_update(0, "end".into(), "abc".into());

        let rows = slots(ui);
        assert_eq!(rows[0].start_hour.as_str(), "99");
        assert_eq!(rows[0].end_hour.as_str(), "abc");
        assert!(rows[0].wraps, "99 >= 0 after… the clamped values wrap");
        assert!(!rows[0].summary.is_empty());
    });
}

#[test]
fn the_media_override_editor_edits_rows_in_place_and_removes_only_the_one_asked_for() {
    with_ui(|ui| {
        ui.window.invoke_open_settings();
        assert!(
            override_rows(ui).is_empty(),
            "nothing is pinned until the user adds a row"
        );

        ui.window.invoke_disk_override_add();
        let rows = override_rows(ui);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].path.is_empty());
        assert_eq!(
            rows[0].media_idx, 1,
            "a new row defaults to HDD — the usual reason to add one"
        );
        let guidance = rows[0].detected_text.clone();
        assert!(
            !guidance.is_empty(),
            "an empty row explains what to type instead of claiming a media type"
        );

        let first = absolute_dir("limedl-override");
        ui.window
            .invoke_disk_override_path_edited(0, first.clone().into());
        let rows = override_rows(ui);
        assert_eq!(rows[0].path.as_str(), first.as_str());
        assert_ne!(
            rows[0].detected_text, guidance,
            "a usable path gets a detection instead of the typing guidance"
        );

        // The picker index is the editor's combo order: 0 = SSD, 1 = HDD.
        ui.window.invoke_disk_override_media_selected(0, 0);
        assert_eq!(override_rows(ui)[0].media_idx, 0);

        // A second row, so removal can be checked for "only this one".
        let second = absolute_dir("limedl-second");
        ui.window.invoke_disk_override_add();
        ui.window
            .invoke_disk_override_path_edited(1, second.clone().into());
        assert_eq!(override_rows(ui).len(), 2);

        ui.window.invoke_disk_override_remove(0);
        let rows = override_rows(ui);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].path.as_str(),
            second.as_str(),
            "the other row survives"
        );
        assert_eq!(rows[0].media_idx, 1, "...with its own picker position");

        // Out-of-range indices are no-ops, not panics: the model and the store
        // can briefly disagree while the list is rebuilt.
        ui.window.invoke_disk_override_remove(9);
        ui.window
            .invoke_disk_override_path_edited(9, "C:\\nowhere".into());
        ui.window.invoke_disk_override_media_selected(9, 0);
        assert_eq!(override_rows(ui).len(), 1);

        // A blank path is not usable as a key, so the hint goes back to
        // "type an absolute path" rather than describing a media type.
        ui.window
            .invoke_disk_override_path_edited(0, "   ".into());
        let rows = override_rows(ui);
        assert_eq!(
            rows[0].path.as_str(),
            "   ",
            "the editor keeps what the user typed"
        );
        assert!(rows[0].detected_text.contains("NAS"));
    });
}

#[test]
fn switching_tabs_does_not_disturb_the_form_or_the_open_state() {
    with_ui(|ui| {
        ui.click("MainWindow::ta_set");
        let mut form = ui.window.get_settings_form();
        form.aria2_port = "1234".into();
        ui.window.set_settings_form(form);

        ui.window.invoke_set_settings_tab(4);
        assert_eq!(ui.window.get_settings_tab(), 4);
        assert!(
            ui.window.get_show_settings(),
            "switching tabs keeps the dialog open"
        );
        assert_eq!(
            ui.window.get_settings_form().aria2_port.as_str(),
            "1234",
            "an unsaved edit must survive a tab switch"
        );

        ui.window.invoke_close_settings();
        assert!(!ui.window.get_show_settings());
    });
}

#[test]
fn the_speed_limit_dialog_closes_on_submit_even_with_unparsable_input() {
    with_ui(|ui| {
        ui.window.invoke_open_speed_limit_dialog();
        assert!(ui.window.get_show_speed_limit_dialog());

        // Non-numeric input parses to `None`, which means "no limit" rather than
        // an error — and the dialog closes either way. This pins that contract;
        // the BT call itself needs a real backend (see the MCP tier).
        ui.window
            .invoke_submit_speed_limit("not-a-number".into(), "0".into());
        assert!(!ui.window.get_show_speed_limit_dialog());
    });
}
