//! Dialog save paths and their rejections: the settings dialog (form, appearance
//! globals), the labs rule editor (merged into the settings on save), the first-run
//! wizard and the factory-reset gate that guards the About tab's one destructive
//! action.

use limedl_core::types::{DiskType, MatchType, ReplacementMode};

use crate::{ColorModePref, Theme, ThemeAccent};

use slint::{ComponentHandle, Model};

use super::super::{Language, TestUi, WindowEvent, absolute_dir, new_window};

pub(super) async fn a_rejected_save_keeps_the_dialog_open() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");

    // Proxy set to "manual" with no URL: the form validator rejects it before the
    // core is even asked.
    let mut form = ui.window.get_settings_form();
    form.proxy_mode_idx = 2;
    form.proxy_manual_url = "".into();
    ui.window.set_settings_form(form);

    ui.window
        .invoke_save_settings(ui.window.get_settings_form());
    ui.pump_until("the rejected save to surface a toast", || {
        !ui.toasts().is_empty()
    })
    .await;

    assert_eq!(ui.toasts()[0].0, "error");
    assert!(
        ui.window.get_show_settings(),
        "a validation error must not close the dialog and lose the edits"
    );
}

pub(super) async fn an_invalid_schedule_row_is_reported() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    ui.window.invoke_schedule_set_enabled(true);
    ui.window
        .invoke_schedule_update(0, "start".into(), "not-a-hour".into());

    ui.window
        .invoke_save_settings(ui.window.get_settings_form());
    ui.pump_until("the schedule validation to surface a toast", || {
        !ui.toasts().is_empty()
    })
    .await;

    assert_eq!(ui.toasts()[0].0, "error");
    assert!(
        ui.window.get_show_settings(),
        "the row the user has to fix must stay reachable"
    );
}

/// The override rows are the second list editor whose text lives in the UI model
/// until Save, and a relative path is the one input the engine can never match
/// (its lookup compares absolute paths, and Windows detection silently answers
/// "SSD" for one).
pub(super) async fn an_invalid_media_override_row_is_reported() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    ui.window.invoke_disk_override_add();
    ui.window
        .invoke_disk_override_path_edited(0, "relative/downloads".into());

    ui.window
        .invoke_save_settings(ui.window.get_settings_form());
    ui.pump_until("the override validation to surface a toast", || {
        !ui.toasts().is_empty()
    })
    .await;

    assert_eq!(ui.toasts()[0].0, "error");
    assert!(
        ui.window.get_show_settings(),
        "the row the user has to fix must stay reachable"
    );
}

/// Saving persisted override rows: the editor's model is the only source of the
/// typed paths, so a successful save is the one place where a row becomes a key
/// the engine can look a download destination up against — and the merge has to
/// keep the user's spelling while mapping the picker back to a media type.
pub(super) async fn saving_media_overrides_persists_the_rows() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");

    // Row 0 keeps the editor's default (HDD — the usual reason to add a row).
    let nas = absolute_dir("limedl-nas");
    let vhd = absolute_dir("limedl-vhd");
    ui.window.invoke_disk_override_add();
    ui.window
        .invoke_disk_override_path_edited(0, nas.clone().into());
    // Row 1 is pinned to SSD, so both directions of the combo mapping are hit.
    ui.window.invoke_disk_override_add();
    ui.window
        .invoke_disk_override_path_edited(1, vhd.clone().into());
    ui.window.invoke_disk_override_media_selected(1, 0);

    ui.click("SettingsDialog::save_btn");
    ui.pump_until("the save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;

    {
        let settings = ui.ctx.current_settings.lock();
        let overrides = &settings.io_baseline.disk_type_overrides;
        assert_eq!(overrides.len(), 2, "both rows must be persisted");
        assert_eq!(
            overrides.get(&nas),
            Some(&DiskType::Hdd),
            "the default picker position forces HDD"
        );
        assert_eq!(overrides.get(&vhd), Some(&DiskType::Ssd));
    }

    // Rebuilding the rows from the saved settings is what the next visit shows.
    ui.pump_until("the dialog to close", || !ui.window.get_show_settings())
        .await;
    ui.click("MainWindow::ta_set");
    let rows = ui.window.get_disk_type_overrides();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).expect("row 0").path.as_str(), nas.as_str());
    assert_eq!(rows.row_data(1).expect("row 1").media_idx, 0);
}

/// Saving the settings dialog: the edited form has to reach `AppSettings`, the
/// engine (one `update_settings` per backend) and the user (dialog closed +
/// confirmation). Only the *rejected* save had a test before, which never
/// exercised the success path at all.
pub(super) async fn saving_settings_persists_the_edited_form() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    assert!(ui.window.get_show_settings());

    // One field, edited the way the dialog edits it. The download directory is
    // deliberate: an aria2/autostart change would fire the side effects the real
    // save configures (service restart, registry write) and this test is about
    // the form → settings → engine path.
    let mut form = ui.window.get_settings_form();
    form.default_download_dir = "C:\\limedl-saved".into();
    // Appearance is the one section whose effect is a *global* instead of a
    // dialog property: the saved mode and accent are what the whole UI is
    // painted with, so they get asserted below.
    form.appearance_color_mode_idx = 2; // combo::COLOR_MODES = system / light / dark
    form.appearance_theme_color_idx = 0; // combo::THEME_COLORS = amber / sky / lime
    ui.window.set_settings_form(form);

    ui.click("SettingsDialog::save_btn");
    ui.pump_until("the save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    assert_eq!(
        ui.ctx.current_settings.lock().download.default_download_dir,
        "C:\\limedl-saved",
        "the edited field must land in the shared settings"
    );

    ui.pump_until("the dialog to close", || !ui.window.get_show_settings())
        .await;
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_settings_saved(Language::ZhCn),
    );

    // The save is also the only place `apply_appearance` runs for a running
    // window, and the mode/accent combo indexes are mapped by hand (three
    // values each, silently falling back to the first one).
    let theme = ui.window.global::<Theme>();
    assert_eq!(theme.get_mode(), ColorModePref::Dark);
    assert_eq!(theme.get_accent(), ThemeAccent::Amber);
    assert!(
        theme.get_dark(),
        "the dark flag is derived from the mode, and every colour token reads it"
    );
}

/// Saving the labs dialog: the rule editor's Rust-side list is what gets
/// persisted (the widget owns the field text until Save), so this is the only
/// place where the two sources of truth are merged.
pub(super) async fn saving_labs_persists_the_rules() {
    let ui = new_window();
    ui.window.invoke_open_labs();
    ui.window.invoke_import_rewrite_preset("github".into());
    assert_eq!(ui.ctx.rewrite_rules.lock().len(), 1);

    // The four select/switch fields are the ones the editor deliberately does
    // *not* repaint (the widget owns the view until Save), so the persisted rule
    // is the only place their values can be checked — and `str_to_match_type` /
    // `str_to_replacement_mode` fall back to the first enum variant for a string
    // they do not know, which is exactly how a renamed `.slint` option value
    // would go unnoticed.
    let (encode_before, fallback_before) = {
        let rules = ui.ctx.rewrite_rules.lock();
        (rules[0].encode_url, rules[0].fallback_to_original)
    };
    ui.window.invoke_update_rule_match_type(0, "regex".into());
    ui.window.invoke_update_rule_mode(0, "template".into());
    ui.window.invoke_toggle_rule_encode(0);
    ui.window.invoke_toggle_rule_fallback(0);

    ui.click("LabsDialog::save_labs_btn");
    ui.pump_until("the labs save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    {
        let settings = ui.ctx.current_settings.lock();
        assert_eq!(
            settings.url_rewrite.rules.len(),
            1,
            "the imported rule must be persisted with the settings"
        );
        let rule = &settings.url_rewrite.rules[0];
        assert_eq!(rule.match_type, MatchType::Regex);
        assert_eq!(rule.replacement_mode, ReplacementMode::Template);
        assert_eq!(rule.encode_url, !encode_before, "the switch flips the flag");
        assert_eq!(
            rule.fallback_to_original, !fallback_before,
            "…and so does the fallback switch"
        );
    }

    ui.pump_until("the dialog to close", || !ui.window.get_show_labs())
        .await;
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_labs_saved(Language::ZhCn),
    );
}

/// The first-run wizard, in three phases: finishing persists the form and marks
/// the setup done, the header close button only remembers the step (and syncs the
/// language the cards switched live), and “re-run setup” resets both flags.
///
/// `factory_reset` is deliberately out of reach here — it shuts the backends
/// down, wipes the data directory and relaunches the process.
pub(super) async fn the_setup_wizard_persists_its_form_and_remembers_where_it_was() {
    // Phase 1: finish from the last step.
    let ui = new_window();
    ui.window.set_setup_start_step(8);
    ui.window.set_show_setup_wizard(true);
    assert!(ui.has("SetupWizard::finish_btn"));

    ui.click("SetupWizard::finish_btn");
    ui.pump_until("the wizard save to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    {
        let settings = ui.ctx.current_settings.lock();
        assert!(
            settings.setup_completed,
            "finishing marks the setup as done"
        );
        assert_eq!(settings.last_setup_step, Some(8));
    }
    ui.pump_until("the wizard to close", || !ui.window.get_show_setup_wizard())
        .await;
    // `assert_toast` (exactly one) would be wrong here: finishing can legitimately
    // emit side-effect toasts on the way, e.g. `sync_aria2_rpc` stopping a
    // service the default form switches off. What the wizard owes the user is the
    // success line.
    assert!(
        ui.toasts().iter().any(|(kind, message)| kind == "success"
            && message == crate::i18n::format_toast_setup_finished(Language::ZhCn)),
        "finishing must confirm: {:?}",
        ui.toasts()
    );

    // Phase 2: the language cards and the header close button.
    let ui = new_window();
    ui.window.set_setup_start_step(1);
    ui.window.set_show_setup_wizard(true);
    // The three language cards share their row, so the `en-US` card is clickable
    // like the others (it used to overflow the modal at every window size).
    assert_eq!(ui.find_all("LanguageCard::ta").len(), 3);
    ui.click_nth("LanguageCard::ta", 2); // zh-CN / zh-TW / en-US, in that order
    assert_eq!(ui.window.get_setup_form().language_idx, 2);
    // `select_bundled_translation` is process-global, so put it back before the
    // assertions below — the fixture's other windows expect zh-CN (`cargo test`
    // runs them in one process, nextest does not).
    crate::i18n::apply_translation(Language::ZhCn);

    ui.click("SetupWizard::wizard_close_btn");
    ui.pump_until("the step to be persisted", || {
        ui.ctx.current_settings.lock().last_setup_step == Some(1)
    })
    .await;
    assert_eq!(
        ui.ctx.store.lock().language(),
        Language::EnUs,
        "closing the wizard hands its live language to the task store"
    );
    ui.pump_until("the wizard to close", || !ui.window.get_show_setup_wizard())
        .await;
    assert!(
        !ui.ctx.current_settings.lock().setup_completed,
        "closing the wizard is not finishing it"
    );

    // Phase 3: “Re-run Setup Wizard” (the About tab's button calls exactly this
    // callback) resets both wizard flags and reopens the first step.
    let ui = new_window();
    {
        let mut settings = ui.ctx.current_settings.lock();
        settings.setup_completed = true;
        settings.last_setup_step = Some(5);
    }
    ui.window.set_show_settings(true);

    ui.window.invoke_restart_setup();
    ui.pump_until("the restart to reach the engine", || {
        ui.core.settings_pushes() == 1
    })
    .await;
    {
        let settings = ui.ctx.current_settings.lock();
        assert!(!settings.setup_completed, "the wizard has to run again");
        assert_eq!(settings.last_setup_step, None, "…from the first step");
    }
    ui.pump_until("the wizard to come back", || {
        ui.window.get_show_setup_wizard()
    })
    .await;
    assert!(!ui.window.get_show_settings());
    assert_eq!(ui.window.get_setup_start_step(), 0);
}

/// Scroll the About tab until the danger zone is fully inside the modal.
///
/// `SettingsTabAbout` is a `ScrollView` and the danger zone is its last section:
/// the dialog is capped at 820px, so it starts below the fold, and the element
/// queries prune whatever the viewport clips (an item outside its clip rect
/// counts as invisible). The wheel scroll has to animate, hence the pump.
async fn scroll_danger_zone_into_view(ui: &TestUi) {
    let (modal_x, modal_y, modal_width, modal_height) = ui.bounds("SettingsDialog::modal");
    let position =
        slint::LogicalPosition::new(modal_x + modal_width / 2.0, modal_y + modal_height / 2.0);
    for _ in 0..8 {
        if ui.has("SettingsTabAbout::reset_arm_btn") {
            let (_, button_y, _, button_height) = ui.bounds("SettingsTabAbout::reset_arm_btn");
            if button_y >= modal_y && button_y + button_height <= modal_y + modal_height {
                return;
            }
        }
        ui.window
            .window()
            .dispatch_event(WindowEvent::PointerScrolled {
                position,
                delta_x: 0.0,
                delta_y: -200.0,
            });
        ui.pump(8).await;
    }
    panic!(
        "the factory-reset button never scrolled into view; visible ids: {:?}",
        ui.ids()
    );
}

/// The About tab's “Factory Reset” is a two-click gate (`reset_confirm`), and it
/// is the only thing between a click and `factory_reset` — which shuts every
/// backend down, wipes the data directory and relaunches the process.
///
/// The confirm button is therefore never clicked. What gets asserted is the gate
/// itself, including the path that used to leave it armed: closing the dialog
/// (Escape, the same handler the header button calls) and coming back — the
/// settings tab is remembered, so the About tab is what the next visit shows.
pub(super) async fn the_factory_reset_gate_disarms_when_the_dialog_closes() {
    let ui = new_window();
    ui.click("MainWindow::ta_set");
    ui.window.invoke_set_settings_tab(8);
    scroll_danger_zone_into_view(&ui).await;

    assert!(!ui.has("SettingsTabAbout::reset_confirm_btn"));
    ui.click("SettingsTabAbout::reset_arm_btn");
    assert!(
        ui.has("SettingsTabAbout::reset_confirm_btn"),
        "the first click only arms the gate"
    );
    assert!(
        !ui.has("SettingsTabAbout::reset_arm_btn"),
        "the two buttons are mutually exclusive, so nothing can re-arm by accident"
    );
    assert!(ui.window.get_reset_confirm());

    ui.click("SettingsTabAbout::reset_cancel_btn");
    assert!(ui.has("SettingsTabAbout::reset_arm_btn"));
    assert!(!ui.window.get_reset_confirm(), "cancel disarms the gate");

    // Arm it again and leave through the dialog's own close path.
    ui.click("SettingsTabAbout::reset_arm_btn");
    ui.window.invoke_handle_key_escape();
    assert!(!ui.window.get_show_settings());
    assert!(
        !ui.window.get_reset_confirm(),
        "closing the dialog must disarm the gate"
    );

    ui.click("MainWindow::ta_set");
    ui.window.invoke_set_settings_tab(8);
    scroll_danger_zone_into_view(&ui).await;
    assert!(
        ui.has("SettingsTabAbout::reset_arm_btn"),
        "reopening the settings must not show the data-destroying button one click away"
    );
    assert!(!ui.has("SettingsTabAbout::reset_confirm_btn"));
}
