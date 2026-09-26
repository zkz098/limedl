//! URL rewrite rule editing: presets, custom rules, targets and the live-test
//! sandbox.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;

use limedl_core::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule};

use crate::bridge::{
    create_url_rewrite_preset, str_to_match_type, str_to_replacement_mode,
    url_rewrite_rules_to_slint,
};
use crate::context::AppContext;
use crate::handlers::common::with_ui;
use crate::handlers::labs::{push_rewrite_state, push_sandbox_result};
use crate::i18n;
use crate::MainWindow;

/// Run `f` with the rule list locked and the window upgraded.
fn with_rules(
    ui: &slint::Weak<MainWindow>,
    rules: &Arc<Mutex<Vec<UrlRewriteRule>>>,
    f: impl FnOnce(&MainWindow, &[UrlRewriteRule]),
) {
    with_ui(ui, |ui| {
        let rules = rules.lock();
        f(&ui, &rules);
    });
}

/// Repaint the rule list (used by edits that cannot change the sandbox result).
fn repaint_rules(
    ui: &slint::Weak<MainWindow>,
    rules: &Arc<Mutex<Vec<UrlRewriteRule>>>,
    expanded: &Arc<Mutex<HashSet<String>>>,
) {
    with_rules(ui, rules, |ui, rules| {
        ui.set_rewrite_rules(url_rewrite_rules_to_slint(rules, &expanded.lock()));
    });
}

/// Repaint the rule list *and* the sandbox (used by edits that change which
/// rule matches).
fn repaint_all(
    ui: &slint::Weak<MainWindow>,
    rules: &Arc<Mutex<Vec<UrlRewriteRule>>>,
    expanded: &Arc<Mutex<HashSet<String>>>,
    test_url: &Arc<Mutex<String>>,
) {
    with_rules(ui, rules, |ui, rules| {
        push_rewrite_state(ui, rules, &expanded.lock(), &test_url.lock());
    });
}

/// Mutate one rule field; out-of-range indices are ignored because the model and
/// the store can briefly disagree while the list is rebuilt.
fn edit_rule(
    rules: &Arc<Mutex<Vec<UrlRewriteRule>>>,
    idx: i32,
    edit: impl FnOnce(&mut UrlRewriteRule),
) {
    let mut rules = rules.lock();
    if let Some(rule) = rules.get_mut(idx as usize) {
        edit(rule);
    }
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let rules = ctx.rewrite_rules.clone();
    let expanded = ctx.labs_expanded_ids.clone();
    let test_url = ctx.sandbox_test_url.clone();
    let store = ctx.store.clone();

    // Presets
    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let test_url = test_url.clone();
        let store = store.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_import_rewrite_preset(move |preset_key| {
            let lang = store.lock().language();
            let Some(rule) = create_url_rewrite_preset(&preset_key, lang) else {
                return;
            };
            {
                let mut guard = rules.lock();
                guard.retain(|existing| existing.name != rule.name);
                guard.push(rule);
            }
            repaint_all(&ui_weak, &rules, &expanded, &test_url);
        });
    }

    // Add / remove rules
    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let store = store.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_add_custom_rule(move || {
            {
                let mut guard = rules.lock();
                let id = format!("rule-{}", uuid::Uuid::new_v4().simple());
                let order = guard.len() as u32;
                expanded.lock().insert(id.clone());
                guard.push(UrlRewriteRule {
                    id,
                    name: i18n::new_rewrite_rule_name(store.lock().language()).to_string(),
                    enabled: true,
                    match_type: MatchType::Host,
                    pattern: "*.example.com".to_string(),
                    replacement_mode: ReplacementMode::PrefixProxy,
                    encode_url: true,
                    fallback_to_original: true,
                    order,
                    targets: vec![RewriteTarget {
                        url_template: "https://mirror.example.com".to_string(),
                        enabled: true,
                        order: 0,
                    }],
                });
            }
            repaint_rules(&ui_weak, &rules, &expanded);
        });
    }

    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let test_url = test_url.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_remove_rule(move |idx| {
            {
                let mut guard = rules.lock();
                if (idx as usize) < guard.len() {
                    let removed = guard.remove(idx as usize);
                    expanded.lock().remove(&removed.id);
                }
            }
            repaint_all(&ui_weak, &rules, &expanded, &test_url);
        });
    }

    // Rule state
    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_toggle_rule_expanded(move |idx| {
            {
                let guard = rules.lock();
                if let Some(rule) = guard.get(idx as usize) {
                    let mut expanded = expanded.lock();
                    if expanded.contains(&rule.id) {
                        expanded.remove(&rule.id);
                    } else {
                        expanded.insert(rule.id.clone());
                    }
                }
            }
            repaint_rules(&ui_weak, &rules, &expanded);
        });
    }

    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let test_url = test_url.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_toggle_rule_enabled(move |idx| {
            edit_rule(&rules, idx, |rule| rule.enabled = !rule.enabled);
            repaint_all(&ui_weak, &rules, &expanded, &test_url);
        });
    }

    // Rule fields
    {
        let rules = rules.clone();
        ui.on_update_rule_name(move |idx, val| {
            edit_rule(&rules, idx, |rule| rule.name = val.to_string());
        });
    }

    {
        let rules = rules.clone();
        ui.on_update_rule_match_type(move |idx, val| {
            edit_rule(&rules, idx, |rule| {
                rule.match_type = str_to_match_type(val.as_str());
            });
        });
    }

    {
        let rules = rules.clone();
        ui.on_update_rule_pattern(move |idx, val| {
            edit_rule(&rules, idx, |rule| rule.pattern = val.to_string());
        });
    }

    {
        let rules = rules.clone();
        ui.on_update_rule_mode(move |idx, val| {
            edit_rule(&rules, idx, |rule| {
                rule.replacement_mode = str_to_replacement_mode(val.as_str());
            });
        });
    }

    {
        let rules = rules.clone();
        ui.on_toggle_rule_encode(move |idx| {
            edit_rule(&rules, idx, |rule| rule.encode_url = !rule.encode_url);
        });
    }

    {
        let rules = rules.clone();
        ui.on_toggle_rule_fallback(move |idx| {
            edit_rule(&rules, idx, |rule| {
                rule.fallback_to_original = !rule.fallback_to_original;
            });
        });
    }

    // Rule targets
    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_add_rule_target(move |idx| {
            edit_rule(&rules, idx, |rule| {
                let order = rule.targets.len() as u32;
                rule.targets.push(RewriteTarget {
                    url_template: "https://mirror.example.com".to_string(),
                    enabled: true,
                    order,
                });
            });
            repaint_rules(&ui_weak, &rules, &expanded);
        });
    }

    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_remove_rule_target(move |ridx, tidx| {
            edit_rule(&rules, ridx, |rule| {
                if (tidx as usize) < rule.targets.len() {
                    rule.targets.remove(tidx as usize);
                }
            });
            repaint_rules(&ui_weak, &rules, &expanded);
        });
    }

    {
        let rules = rules.clone();
        ui.on_update_rule_target(move |ridx, tidx, val| {
            edit_rule(&rules, ridx, |rule| {
                if let Some(target) = rule.targets.get_mut(tidx as usize) {
                    target.url_template = val.to_string();
                }
            });
        });
    }

    {
        let rules = rules.clone();
        let expanded = expanded.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_toggle_rule_target(move |ridx, tidx| {
            edit_rule(&rules, ridx, |rule| {
                if let Some(target) = rule.targets.get_mut(tidx as usize) {
                    target.enabled = !target.enabled;
                }
            });
            repaint_rules(&ui_weak, &rules, &expanded);
        });
    }

    // Live test sandbox
    {
        let rules = rules.clone();
        let test_url = test_url.clone();
        let ui_weak = ctx.ui_weak.clone();
        ui.on_test_url_changed(move |val| {
            *test_url.lock() = val.to_string();
            with_rules(&ui_weak, &rules, |ui, rules| {
                push_sandbox_result(ui, rules, &val);
            });
        });
    }
}
