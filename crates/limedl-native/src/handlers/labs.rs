use std::net::IpAddr;
use std::time::Duration;
use slint::{ComponentHandle, SharedString};

use limedl_core::types::{MatchType, ReplacementMode, RewriteTarget, UrlRewriteRule};

use crate::context::AppContext;
use crate::bridge::{
    cdn_candidates_to_slint, create_url_rewrite_preset, evaluate_url_rewrite, format_timestamp_ms,
    str_to_match_type, str_to_replacement_mode, update_app_settings_from_labs_form,
    url_rewrite_rules_to_slint,
};
use crate::i18n::{self, Language};
use crate::toast::push_toast;
use crate::ui_sync::refresh_labs_state;

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let event_bus = ctx.event_bus.clone();
    let store = ctx.store.clone();
    let current_settings = ctx.current_settings.clone();
    let toast_queue = ctx.toast_queue.clone();
    let expanded_rule_ids = ctx.labs_expanded_ids.clone();
    let cdn_candidates_cache = ctx.labs_candidates.clone();
    let rewrite_rules = ctx.rewrite_rules.clone();
    let sandbox_test_url = ctx.sandbox_test_url.clone();

    // Open Labs Dialog
    {
        let current_settings_clone = current_settings.clone();
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_open_labs(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let settings = current_settings_clone.lock().clone();
                let rules = rewrite_rules_clone.lock().clone();
                let exp = expanded_rule_ids_clone.lock().clone();
                let url = sandbox_test_url_clone.lock().clone();
                let cands = cdn_candidates_cache_clone.lock().clone();
                let lang = store_clone.lock().language();
                refresh_labs_state(&ui, &settings, &rules, &exp, &url, false, &cands, lang);
                ui.set_show_labs(true);
            }
        });
    }

    // Close Labs Dialog
    {
        let ui_weak = main_window.as_weak();
        main_window.on_close_labs(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_labs(false);
            }
        });
    }

    // Switch Labs Tab
    {
        let ui_weak = main_window.as_weak();
        main_window.on_set_labs_tab(move |tab| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_labs_tab(tab);
            }
        });
    }

    // Save Labs Configuration
    {
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let rewrite_rules_clone = rewrite_rules.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_save_labs(move |form_data| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let rewrite_rules_clone = rewrite_rules_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let mut settings = {
                    let s = current_settings_clone.lock();
                    s.clone()
                };
                let lang = store_clone.lock().language();

                update_app_settings_from_labs_form(&mut settings, &form_data);
                settings.url_rewrite.rules = rewrite_rules_clone.lock().clone();

                match dispatcher.save_settings(&settings).await {
                    Ok(saved) => {
                        *current_settings_clone.lock() = saved;
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_labs_saved(lang).to_string(),
                            "success",
                            Duration::from_secs(4),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_show_labs(false);
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!("保存实验室设置失败: {err:#}");
                        let msg = format!("{err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_labs_save_failed(&msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Start CDN Speed Test
    {
        let dispatcher = dispatcher.clone();
        let event_bus = event_bus.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_start_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let event_bus = event_bus.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue_clone = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            let form = if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                let current_lang = store_clone.lock().language();
                form.cdn_is_testing = true;
                form.cdn_status_type = SharedString::from("testing");
                form.cdn_status_label = SharedString::from(match current_lang {
                    Language::ZhCn => "测速中",
                    Language::ZhTw => "測速中",
                    Language::EnUs => "Testing",
                });
                form.cdn_phase_label = SharedString::from(match current_lang {
                    Language::ZhCn => "获取网段",
                    Language::ZhTw => "獲取網段",
                    Language::EnUs => "Fetching IP ranges",
                });
                form.cdn_progress_percent = 0.0;
                form.cdn_progress_label = SharedString::from("0 / 0");
                form.cdn_last_error = SharedString::default();
                ui.set_labs_form(form.clone());
                form
            } else {
                return;
            };

            let mut settings = current_settings_clone.lock().clone();
            update_app_settings_from_labs_form(&mut settings, &form);

            tokio::spawn(async move {
                let Some(cs) = dispatcher.cdn_service() else {
                    return;
                };
                let cs = cs.clone();

                match cs.start_test(settings).await {
                    Ok(()) => {
                        let outcome = cs.monitor_test(event_bus).await;

                        let now_ms = limedl_core::now_ms();
                        if let Ok(mut current) = dispatcher.get_settings().await {
                            use limedl_core::cdn::accelerator::AccelState;
                            match &outcome.state {
                                AccelState::Ready => {
                                    current.cdn_acceleration.active_ip =
                                        outcome.active_ip.map(|i| i.to_string());
                                    current.cdn_acceleration.active_speed_mbps =
                                        outcome.active_speed_mbps;
                                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                                    current.cdn_acceleration.last_error = None;
                                }
                                AccelState::Error(msg) => {
                                    current.cdn_acceleration.last_error = Some(msg.clone());
                                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                                }
                                _ => {}
                            }
                            if let Ok(saved) = dispatcher.save_settings(&current).await {
                                *current_settings_clone.lock() = saved;
                            }
                        }

                        *cdn_candidates_cache_clone.lock() = outcome.candidates.clone();
                        let active_ip_str = outcome
                            .active_ip
                            .map(|i| i.to_string())
                            .unwrap_or_default();
                        let cands = outcome.candidates.clone();

                        let default_node_text = outcome.default_node.as_ref().and_then(|dn| {
                            dn.ip.as_deref().map(|ip| {
                                if let Some(spd) = dn.throughput_mbps {
                                    format!("{ip} ({spd:.2} MB/s)")
                                } else if dn.tcp_latency_ms > 0.0 {
                                    format!("{ip} ({:.1} ms)", dn.tcp_latency_ms)
                                } else {
                                    ip.to_string()
                                }
                            })
                        });

                        let speed_improvement = match (
                            outcome.active_speed_mbps,
                            outcome.default_node.as_ref().and_then(|dn| dn.throughput_mbps),
                        ) {
                            (Some(act), Some(base)) if base > 0.0 => {
                                let diff = (act - base) / base * 100.0;
                                Some(format!("{diff:+.1}%"))
                            }
                            _ => None,
                        };

                        let best_latency = outcome
                            .candidates
                            .iter()
                            .map(|c| c.tcp_latency_ms)
                            .fold(f64::INFINITY, f64::min);
                        let latency_improvement = match (
                            best_latency,
                            outcome.default_node.as_ref().map(|dn| dn.tcp_latency_ms),
                        ) {
                            (act, Some(base)) if act.is_finite() && base > 0.0 => {
                                let diff = (act - base) / base * 100.0;
                                Some(format!("{diff:+.1}%"))
                            }
                            _ => None,
                        };

                        let last_test_time = format_timestamp_ms(now_ms);
                        let is_ready = matches!(outcome.state, limedl_core::cdn::accelerator::AccelState::Ready);

                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                if let Some(ref text) = default_node_text {
                                    form.cdn_default_node_text = SharedString::from(text);
                                }
                                if let Some(ref text) = speed_improvement {
                                    form.cdn_speed_improvement_text = SharedString::from(text);
                                }
                                if let Some(ref text) = latency_improvement {
                                    form.cdn_latency_improvement_text = SharedString::from(text);
                                }
                                if is_ready {
                                    form.cdn_last_test_time = SharedString::from(last_test_time);
                                }
                                ui.set_labs_form(form);
                                ui.set_cdn_candidates(cdn_candidates_to_slint(
                                    &cands,
                                    &active_ip_str,
                                ));
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("启动 CDN 测速失败: {e:#}");
                        let err_msg = e.to_string();
                        let current_lang = store_clone.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue_clone,
                            i18n::format_toast_cdn_test_failed(&err_msg, current_lang),
                            "error",
                            Duration::from_secs(6),
                        );
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                form.cdn_status_type = SharedString::from("error");
                                form.cdn_status_label = SharedString::from(match current_lang {
                                    Language::ZhCn => "测速失败",
                                    Language::ZhTw => "測速失敗",
                                    Language::EnUs => "Failed",
                                });
                                form.cdn_last_error = SharedString::from(err_msg);
                                ui.set_labs_form(form);
                            }
                        });
                    }
                }
            });
        });
    }

    // Cancel CDN Speed Test
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_cancel_cdn_test(move || {
            if let Some(cs) = dispatcher.cdn_service() {
                cs.cancel_test();
                let current_lang = store_clone.lock().language();
                if let Some(ui) = ui_weak.upgrade() {
                    let mut form = ui.get_labs_form();
                    form.cdn_is_testing = false;
                    form.cdn_status_type = SharedString::from("idle");
                    form.cdn_status_label = SharedString::from(match current_lang {
                        Language::ZhCn => "已取消",
                        Language::ZhTw => "已取消",
                        Language::EnUs => "Cancelled",
                    });
                    ui.set_labs_form(form);
                }
            }
        });
    }

    // Clear CDN Speed Test State
    {
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_clear_cdn_test(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                if let Some(cs) = dispatcher.cdn_service() {
                    cs.clear().await;
                    cdn_candidates_cache_clone.lock().clear();
                    let lang = store_clone.lock().language();
                    let mut settings = current_settings_clone.lock().clone();
                    settings.cdn_acceleration.active_ip = None;
                    settings.cdn_acceleration.active_speed_mbps = None;
                    settings.cdn_acceleration.last_test_at_ms = None;
                    settings.cdn_acceleration.last_error = None;
                    if let Ok(saved) = dispatcher.save_settings(&settings).await {
                        *current_settings_clone.lock() = saved;
                    }
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_cdn_cleared(lang).to_string(),
                        "info",
                        Duration::from_secs(4),
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut form = ui.get_labs_form();
                            form.cdn_active_ip = SharedString::default();
                            form.cdn_active_speed_text = SharedString::default();
                            form.cdn_status_type = SharedString::from("idle");
                            form.cdn_status_label = SharedString::from(i18n::cdn_idle_label(lang));
                            ui.set_labs_form(form);
                            ui.set_cdn_candidates(cdn_candidates_to_slint(&[], ""));
                        }
                    });
                }
            });
        });
    }

    // Apply CDN Candidate IP
    {
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_apply_cdn_candidate(move |ip_str, speed_mbps| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let ip_parsed = ip_str.parse::<IpAddr>();

            tokio::spawn(async move {
                if let (Some(cs), Ok(ip)) = (dispatcher.cdn_service(), ip_parsed) {
                    let lang = store_clone.lock().language();
                    let settings = current_settings_clone.lock().clone();
                    if let Ok(()) = cs.apply_ip(ip, speed_mbps as f64, &settings).await {
                        let mut updated_settings = settings.clone();
                        updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                        updated_settings.cdn_acceleration.active_speed_mbps = Some(speed_mbps as f64);
                        if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                            *current_settings_clone.lock() = saved;
                        }

                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_cdn_applied(&ip.to_string(), lang),
                            "success",
                            Duration::from_secs(4),
                        );

                        let cands = cdn_candidates_cache_clone.lock().clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut form = ui.get_labs_form();
                                form.cdn_active_ip = SharedString::from(ip.to_string());
                                form.cdn_active_speed_text = SharedString::from(format!("{speed_mbps:.2} MB/s"));
                                form.cdn_status_type = SharedString::from("ready");
                                form.cdn_status_label = SharedString::from(i18n::cdn_ready_label(lang));
                                ui.set_labs_form(form);
                                ui.set_cdn_candidates(cdn_candidates_to_slint(&cands, &ip.to_string()));
                            }
                        });
                    }
                }
            });
        });
    }

    // Toggle CDN Advanced Panel
    {
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_cdn_advanced(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                form.cdn_show_advanced = !form.cdn_show_advanced;
                ui.set_labs_form(form);
            }
        });
    }

    // Apply Manual CDN IP
    {
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        let cdn_candidates_cache_clone = cdn_candidates_cache.clone();
        let store_cl = store.clone();
        let toast_queue_cl = toast_queue.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_apply_manual_cdn_ip(move |ip_str| {
            let ip_trimmed = ip_str.trim().to_string();
            match ip_trimmed.parse::<IpAddr>() {
                Ok(ip) => {
                    let dispatcher = dispatcher.clone();
                    let current_settings_clone = current_settings_clone.clone();
                    let cdn_candidates_cache_clone = cdn_candidates_cache_clone.clone();
                    let store_clone = store_cl.clone();
                    let toast_queue_clone = toast_queue_cl.clone();
                    let ui_weak = ui_weak.clone();

                    tokio::spawn(async move {
                        if let Some(cs) = dispatcher.cdn_service() {
                            let lang = store_clone.lock().language();
                            let settings = current_settings_clone.lock().clone();
                            if let Ok(()) = cs.apply_ip(ip, 0.0, &settings).await {
                                let mut updated_settings = settings.clone();
                                updated_settings.cdn_acceleration.active_ip = Some(ip.to_string());
                                if let Ok(saved) = dispatcher.save_settings(&updated_settings).await {
                                    *current_settings_clone.lock() = saved;
                                }

                                push_toast(
                                    &ui_weak,
                                    &toast_queue_clone,
                                    i18n::format_toast_cdn_applied(&ip.to_string(), lang),
                                    "success",
                                    Duration::from_secs(4),
                                );

                                let cands = cdn_candidates_cache_clone.lock().clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let mut form = ui.get_labs_form();
                                        form.cdn_active_ip = SharedString::from(ip.to_string());
                                        form.cdn_manual_ip_error = SharedString::default();
                                        form.cdn_status_type = SharedString::from("ready");
                                        form.cdn_status_label = SharedString::from(i18n::cdn_ready_label(lang));
                                        ui.set_labs_form(form);
                                        ui.set_cdn_candidates(cdn_candidates_to_slint(&cands, &ip.to_string()));
                                    }
                                });
                            }
                        }
                    });
                }
                Err(_) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        let mut form = ui.get_labs_form();
                        form.cdn_manual_ip_error =
                            SharedString::from(i18n::format_invalid_ip(store_cl.lock().language()));
                        ui.set_labs_form(form);
                    }
                }
            }
        });
    }

    // ── URL Rewrite Rule Callbacks ──────────────────────────────────────

    // Import Presets
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_import_rewrite_preset(move |preset_key| {
            let lang = store_clone.lock().language();
            if let Some(rule) = create_url_rewrite_preset(&preset_key, lang) {
                let mut rules = rewrite_rules_clone.lock();
                rules.retain(|r| r.name != rule.name);
                rules.push(rule);

                if let Some(ui) = ui_weak.upgrade() {
                    let exp = expanded_rule_ids_clone.lock();
                    let test_url = sandbox_test_url_clone.lock();
                    let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                    let mut form = ui.get_labs_form();
                    form.url_rewrite_test_matched_rule = SharedString::from(matched);
                    form.url_rewrite_test_candidates_count = cands.len() as i32;
                    form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                    form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                    form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                    ui.set_labs_form(form);
                    ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
                }
            }
        });
    }

    // Add Custom Rule
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_add_custom_rule(move || {
            let mut rules = rewrite_rules_clone.lock();
            let id = format!("rule-{}", uuid::Uuid::new_v4().simple());
            let order = rules.len() as u32;
            expanded_rule_ids_clone.lock().insert(id.clone());
            rules.push(UrlRewriteRule {
                id,
                name: i18n::new_rewrite_rule_name(store_clone.lock().language()).to_string(),
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
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Remove Rule
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_remove_rule(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if (idx as usize) < rules.len() {
                let removed = rules.remove(idx as usize);
                expanded_rule_ids_clone.lock().remove(&removed.id);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                let test_url = sandbox_test_url_clone.lock();
                let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Toggle Rule Expanded
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_rule_expanded(move |idx| {
            let rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get(idx as usize) {
                let mut exp = expanded_rule_ids_clone.lock();
                if exp.contains(&rule.id) {
                    exp.remove(&rule.id);
                } else {
                    exp.insert(rule.id.clone());
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Toggle Rule Enabled
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_toggle_rule_enabled(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.enabled = !rule.enabled;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                let test_url = sandbox_test_url_clone.lock();
                let (matched, cands) = evaluate_url_rewrite(&rules, &test_url);
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Update Rule Fields
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_name(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.name = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_match_type(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.match_type = str_to_match_type(val.as_str());
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_pattern(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.pattern = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_mode(move |idx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.replacement_mode = str_to_replacement_mode(val.as_str());
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_toggle_rule_encode(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.encode_url = !rule.encode_url;
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_toggle_rule_fallback(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                rule.fallback_to_original = !rule.fallback_to_original;
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_add_rule_target(move |idx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(idx as usize) {
                let order = rule.targets.len() as u32;
                rule.targets.push(RewriteTarget {
                    url_template: "https://mirror.example.com".to_string(),
                    enabled: true,
                    order,
                });
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_remove_rule_target(move |ridx, tidx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && (tidx as usize) < rule.targets.len()
            {
                rule.targets.remove(tidx as usize);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        main_window.on_update_rule_target(move |ridx, tidx, val| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && let Some(target) = rule.targets.get_mut(tidx as usize)
            {
                target.url_template = val.to_string();
            }
        });
    }
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let expanded_rule_ids_clone = expanded_rule_ids.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_rule_target(move |ridx, tidx| {
            let mut rules = rewrite_rules_clone.lock();
            if let Some(rule) = rules.get_mut(ridx as usize)
                && let Some(target) = rule.targets.get_mut(tidx as usize)
            {
                target.enabled = !target.enabled;
            }
            if let Some(ui) = ui_weak.upgrade() {
                let exp = expanded_rule_ids_clone.lock();
                ui.set_rewrite_rules(url_rewrite_rules_to_slint(&rules, &exp));
            }
        });
    }

    // Live Test Sandbox Input Changed
    {
        let rewrite_rules_clone = rewrite_rules.clone();
        let sandbox_test_url_clone = sandbox_test_url.clone();
        let ui_weak = main_window.as_weak();

        main_window.on_test_url_changed(move |val| {
            *sandbox_test_url_clone.lock() = val.to_string();
            let rules = rewrite_rules_clone.lock();
            let (matched, cands) = evaluate_url_rewrite(&rules, &val);

            if let Some(ui) = ui_weak.upgrade() {
                let mut form = ui.get_labs_form();
                form.url_rewrite_test_matched_rule = SharedString::from(matched);
                form.url_rewrite_test_candidates_count = cands.len() as i32;
                form.url_rewrite_test_result_1 = SharedString::from(cands.first().cloned().unwrap_or_default());
                form.url_rewrite_test_result_2 = SharedString::from(cands.get(1).cloned().unwrap_or_default());
                form.url_rewrite_test_result_3 = SharedString::from(cands.get(2).cloned().unwrap_or_default());
                ui.set_labs_form(form);
            }
        });
    }

}
