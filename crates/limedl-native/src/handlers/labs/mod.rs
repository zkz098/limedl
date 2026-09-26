//! Labs dialog callbacks (CDN acceleration + URL rewrite rules).
//!
//! `register` used to be a single ~800-line function; it is now a table of
//! contents over the modules below, with the two shared repaint helpers kept
//! here because both `dialog` and `rewrite` need them.

mod cdn;
mod dialog;
mod rewrite;

use std::collections::HashSet;

use slint::SharedString;

use limedl_core::types::UrlRewriteRule;

use crate::bridge::{evaluate_url_rewrite, url_rewrite_rules_to_slint};
use crate::context::AppContext;
use crate::MainWindow;

/// Wire every Labs callback onto the main window.
pub fn register(ctx: &AppContext) {
    dialog::register(ctx);
    cdn::register(ctx);
    rewrite::register(ctx);
}

/// Fill the live-test sandbox: the matched rule plus the first three rewritten
/// URLs. Used by every rule edit, which is why it lives here.
pub(super) fn push_sandbox_result(ui: &MainWindow, rules: &[UrlRewriteRule], test_url: &str) {
    let (matched, candidates) = evaluate_url_rewrite(rules, test_url);
    let mut form = ui.get_labs_form();
    form.url_rewrite_test_matched_rule = SharedString::from(matched);
    form.url_rewrite_test_candidates_count = candidates.len() as i32;
    form.url_rewrite_test_result_1 =
        SharedString::from(candidates.first().cloned().unwrap_or_default());
    form.url_rewrite_test_result_2 =
        SharedString::from(candidates.get(1).cloned().unwrap_or_default());
    form.url_rewrite_test_result_3 =
        SharedString::from(candidates.get(2).cloned().unwrap_or_default());
    ui.set_labs_form(form);
}

/// Repaint the sandbox and the rule list after an edit that changes the order,
/// the enabled flag or the rule set itself.
pub(super) fn push_rewrite_state(
    ui: &MainWindow,
    rules: &[UrlRewriteRule],
    expanded: &HashSet<String>,
    test_url: &str,
) {
    push_sandbox_result(ui, rules, test_url);
    ui.set_rewrite_rules(url_rewrite_rules_to_slint(rules, expanded));
}
