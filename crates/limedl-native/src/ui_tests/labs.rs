//! Labs: the URL-rewrite rule editor and the CDN tab's inline validation.
//!
//! The rule editor is the most intricate form in the app — sixteen callbacks
//! mutate a `Vec<UrlRewriteRule>` behind a UI model that is only partially
//! repainted — so the tests here are mostly about *index bookkeeping*: which
//! rule an edit lands on after a neighbour was deleted, whether the expanded
//! state follows the rule or the row, and what happens to an index that no
//! longer exists.
//!
//! Two sources of truth, on purpose (and worth knowing before writing more of
//! these): adds/removes/toggles repaint the UI model, while plain field edits
//! only mutate the Rust list — the widget owns the text until Save. So the
//! assertions below read `ctx.rewrite_rules` for field values and
//! `get_rewrite_rules()` for row presence, expansion and enabled state.

use limedl_core::types::{MatchType, ReplacementMode, UrlRewriteRule};

use super::*;

fn rust_rules(ui: &TestUi) -> Vec<UrlRewriteRule> {
    ui.ctx.rewrite_rules.lock().clone()
}

#[test]
fn adding_rules_expands_them_and_removing_one_keeps_the_rest_in_order() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        assert!(ui.find_all("LabsDialog::modal").len() == 1);

        for _ in 0..3 {
            ui.window.invoke_add_custom_rule();
        }
        let model = ui.window.get_rewrite_rules();
        assert_eq!(model.row_count(), 3, "each add appends a row");
        let rows: Vec<_> = (0..model.row_count())
            .filter_map(|i| model.row_data(i))
            .collect();
        assert!(
            rows.iter().all(|row| row.is_expanded),
            "a freshly added rule opens expanded so it can be edited"
        );
        assert!(rows.iter().all(|row| row.enabled));

        // Tag every rule through the edit callbacks, then delete the middle one.
        ui.window
            .invoke_update_rule_pattern(0, "first.example".into());
        ui.window
            .invoke_update_rule_pattern(1, "second.example".into());
        ui.window
            .invoke_update_rule_pattern(2, "third.example".into());
        assert_eq!(
            rust_rules(ui)
                .iter()
                .map(|rule| rule.pattern.clone())
                .collect::<Vec<_>>(),
            ["first.example", "second.example", "third.example"]
        );

        ui.window.invoke_remove_rule(1);
        assert_eq!(ui.window.get_rewrite_rules().row_count(), 2);
        assert_eq!(
            rust_rules(ui)
                .iter()
                .map(|rule| rule.pattern.clone())
                .collect::<Vec<_>>(),
            ["first.example", "third.example"],
            "removing a rule must not shift the patterns of the remaining ones"
        );
    });
}

#[test]
fn the_expanded_state_follows_the_rule_and_not_the_row() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        ui.window.invoke_add_custom_rule();
        ui.window.invoke_add_custom_rule();
        let ids: Vec<String> = rust_rules(ui).iter().map(|rule| rule.id.clone()).collect();

        // Collapse the first rule, then delete its neighbour.
        ui.window.invoke_toggle_rule_expanded(0);
        ui.window.invoke_remove_rule(1);

        assert!(
            !ui.ctx.labs_expanded_ids.lock().contains(&ids[0]),
            "the collapsed rule must stay collapsed"
        );
        assert!(
            !ui.window
                .get_rewrite_rules()
                .row_data(0)
                .unwrap()
                .is_expanded,
            "…and the repainted row must agree"
        );
    });
}

#[test]
fn out_of_range_indices_are_ignored_instead_of_panicking() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        ui.window.invoke_add_custom_rule();
        let before = rust_rules(ui);

        // The model and the Rust list can briefly disagree while a row is
        // rebuilt, so every callback has to tolerate an index that is gone.
        ui.window.invoke_remove_rule(99);
        ui.window.invoke_toggle_rule_enabled(99);
        ui.window.invoke_toggle_rule_expanded(-1);
        ui.window.invoke_update_rule_name(99, "nope".into());
        ui.window.invoke_update_rule_pattern(-3, "nope".into());
        ui.window.invoke_add_rule_target(99);
        ui.window.invoke_remove_rule_target(0, 99);
        ui.window.invoke_toggle_rule_target(0, 99);
        ui.window.invoke_update_rule_target(0, 99, "nope".into());

        assert_eq!(
            rust_rules(ui),
            before,
            "no out-of-range call may change a rule"
        );
        assert_eq!(ui.window.get_rewrite_rules().row_count(), 1);
    });
}

#[test]
fn targets_can_be_added_toggled_and_removed_down_to_none() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        ui.window.invoke_add_custom_rule();
        let initial = rust_rules(ui)[0].targets.len();
        assert_eq!(initial, 1, "a custom rule starts with one mirror target");

        ui.window.invoke_add_rule_target(0);
        ui.window.invoke_add_rule_target(0);
        assert_eq!(rust_rules(ui)[0].targets.len(), 3);

        ui.window.invoke_toggle_rule_target(0, 1);
        assert!(!rust_rules(ui)[0].targets[1].enabled);
        let repainted = ui.window.get_rewrite_rules();
        assert!(
            !repainted
                .row_data(0)
                .unwrap()
                .targets
                .row_data(1)
                .unwrap()
                .enabled,
            "toggling a target repaints the row"
        );

        ui.window
            .invoke_update_rule_target(0, 1, "https://mirror.test".into());
        assert_eq!(
            rust_rules(ui)[0].targets[1].url_template,
            "https://mirror.test"
        );

        // Unlike the download file list, the target list is allowed to empty:
        // "no mirrors" is a valid (if useless) rule, not data loss.
        for _ in 0..3 {
            ui.window.invoke_remove_rule_target(0, 0);
        }
        assert!(rust_rules(ui)[0].targets.is_empty());
        assert_eq!(
            ui.window
                .get_rewrite_rules()
                .row_data(0)
                .unwrap()
                .targets
                .row_count(),
            0
        );
    });
}

#[test]
fn the_sandbox_preview_follows_the_rules_and_their_enabled_state() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        assert!(
            ui.window
                .get_labs_form()
                .url_rewrite_test_matched_rule
                .is_empty()
        );

        ui.window.invoke_import_rewrite_preset("github".into());
        let rule_name = rust_rules(ui)[0].name.clone();

        ui.window
            .invoke_test_url_changed("https://github.com/user/repo/archive/main.zip".into());
        let form = ui.window.get_labs_form();
        assert_eq!(
            form.url_rewrite_test_matched_rule.as_str(),
            rule_name,
            "a github URL must report the github preset as the matching rule"
        );
        assert!(
            form.url_rewrite_test_candidates_count >= 1,
            "a matching rule must produce candidate mirror URLs, got {}",
            form.url_rewrite_test_candidates_count
        );
        assert!(!form.url_rewrite_test_result_1.is_empty());

        // A URL no rule matches clears the *rule* part of the preview. The
        // candidate list still holds the original URL: `rewrite_url` keeps the
        // source as a fallback so the copy button always has something to offer.
        ui.window
            .invoke_test_url_changed("https://example.org/file.zip".into());
        let form = ui.window.get_labs_form();
        assert!(form.url_rewrite_test_matched_rule.is_empty());
        assert_eq!(
            form.url_rewrite_test_candidates_count, 1,
            "just the original URL"
        );

        // Disabled rules are skipped by the preview, which is what the toggle is
        // for: users compare "with and without" the rewrite.
        ui.window.invoke_toggle_rule_enabled(0);
        ui.window
            .invoke_test_url_changed("https://github.com/user/repo/archive/main.zip".into());
        assert!(
            ui.window
                .get_labs_form()
                .url_rewrite_test_matched_rule
                .is_empty(),
            "a disabled rule must not match"
        );
    });
}

#[test]
fn an_invalid_manual_cdn_ip_reports_inline_and_reopening_labs_clears_it() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        ui.window.invoke_apply_manual_cdn_ip("not-an-ip".into());

        let form = ui.window.get_labs_form();
        assert!(
            !form.cdn_manual_ip_error.is_empty(),
            "an unparsable address must surface as an inline error"
        );

        // The error is transient UI state: the dialog rebuilds the form from the
        // settings on open, so it must not linger into the next visit.
        ui.window.invoke_close_labs();
        assert!(!ui.window.get_show_labs());
        ui.window.invoke_open_labs();
        assert!(ui.window.get_labs_form().cdn_manual_ip_error.is_empty());

        let advanced_before = ui.window.get_labs_form().cdn_show_advanced;
        ui.window.invoke_toggle_cdn_advanced();
        assert_eq!(
            ui.window.get_labs_form().cdn_show_advanced,
            !advanced_before
        );
    });
}

/// The four select/switch callbacks the index tests above never touch.
///
/// They only mutate the Rust rule — the widget owns the view until Save — and
/// `str_to_match_type` / `str_to_replacement_mode` fall back to the first enum
/// variant for a string they do not know, so a renamed `.slint` option value
/// would quietly change what a rule means. The strings asserted here are the ones
/// the dialog's `CustomSelect` models carry.
#[test]
fn the_rule_selects_and_switches_reach_the_rust_rule() {
    with_ui(|ui| {
        ui.window.invoke_open_labs();
        ui.window.invoke_add_custom_rule();
        let before = rust_rules(ui)[0].clone();
        assert_eq!(
            before.match_type,
            MatchType::Host,
            "a new rule matches hosts"
        );
        assert_eq!(before.replacement_mode, ReplacementMode::PrefixProxy);
        assert!(before.encode_url);
        assert!(before.fallback_to_original);

        ui.window.invoke_update_rule_match_type(0, "regex".into());
        ui.window.invoke_update_rule_mode(0, "template".into());
        ui.window.invoke_toggle_rule_encode(0);
        ui.window.invoke_toggle_rule_fallback(0);

        let rule = rust_rules(ui)[0].clone();
        assert_eq!(rule.match_type, MatchType::Regex);
        assert_eq!(rule.replacement_mode, ReplacementMode::Template);
        assert_eq!(rule.encode_url, !before.encode_url, "the switch flips");
        assert_eq!(rule.fallback_to_original, !before.fallback_to_original);

        // The remaining values of both selects, so every option the dialog
        // offers has been mapped at least once.
        for (value, expected) in [
            ("prefix", MatchType::Prefix),
            ("wildcard", MatchType::Wildcard),
            ("host", MatchType::Host),
        ] {
            ui.window.invoke_update_rule_match_type(0, value.into());
            assert_eq!(rust_rules(ui)[0].match_type, expected, "{value}");
        }
        ui.window.invoke_update_rule_mode(0, "prefix_proxy".into());
        assert_eq!(
            rust_rules(ui)[0].replacement_mode,
            ReplacementMode::PrefixProxy
        );

        // A value no select offers must not invent a mode (it lands on the first
        // variant, which is the documented fail-open behavior).
        ui.window
            .invoke_update_rule_match_type(0, "nonsense".into());
        assert_eq!(rust_rules(ui)[0].match_type, MatchType::Host);
    });
}
