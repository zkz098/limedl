//! Settings: the speed-limit schedule editor, the dialog's own state, and what
//! a rejected save leaves behind.
//!
//! `collect_settings` (and therefore the whole save path) lives behind a
//! `tokio::spawn`, so those two tests are `with_ui_async` + `pump_until`. The
//! schedule editor itself is fully synchronous and needs no pumping.

use crate::SpeedLimitSlotItem;
use slint::Model;

use super::*;

fn slots(ui: &TestUi) -> Vec<SpeedLimitSlotItem> {
    let model = ui.window.get_speed_limit_slots();
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .collect()
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
