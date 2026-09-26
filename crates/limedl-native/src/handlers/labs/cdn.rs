//! CDN acceleration callbacks: speedtest lifecycle, node selection and the
//! manual IP override.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use slint::SharedString;

use limedl_core::cdn::CdnTestOutcome;
use limedl_core::cdn::accelerator::AccelState;
use limedl_core::cdn::speed_test::SpeedTestResult;
use limedl_core::dispatcher::Dispatcher;
use limedl_core::event_bus::EventBus;
use limedl_core::types::AppSettings;

use crate::bridge::{
    TaskStore, cdn_candidates_to_slint, format_timestamp_ms, update_app_settings_from_labs_form,
};
use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::i18n::{self, Language};
use crate::toast::{ToastQueue, push_toast};
use crate::MainWindow;

/// Handles shared by the CDN tab. Cloned once per callback so each closure
/// captures one cheap handle instead of six.
#[derive(Clone)]
struct Cdn {
    ui_weak: slint::Weak<MainWindow>,
    dispatcher: Arc<Dispatcher>,
    event_bus: Arc<EventBus>,
    store: Arc<Mutex<TaskStore>>,
    current_settings: Arc<Mutex<AppSettings>>,
    candidates: Arc<Mutex<Vec<SpeedTestResult>>>,
    toast_queue: ToastQueue,
}

/// "直连 DNS (基准)" baseline description for the finished speedtest.
fn default_node_text(outcome: &CdnTestOutcome) -> Option<String> {
    outcome.default_node.as_ref().and_then(|node| {
        node.ip.as_deref().map(|ip| {
            if let Some(throughput) = node.throughput_mbps {
                format!("{ip} ({throughput:.2} MB/s)")
            } else if node.tcp_latency_ms > 0.0 {
                format!("{ip} ({:.1} ms)", node.tcp_latency_ms)
            } else {
                ip.to_string()
            }
        })
    })
}

/// Throughput delta against the direct-DNS baseline, e.g. `+182.4%`.
fn speed_improvement_text(outcome: &CdnTestOutcome) -> Option<String> {
    match (
        outcome.active_speed_mbps,
        outcome.default_node.as_ref().and_then(|node| node.throughput_mbps),
    ) {
        (Some(active), Some(base)) if base > 0.0 => {
            Some(format!("{:+.1}%", (active - base) / base * 100.0))
        }
        _ => None,
    }
}

/// Latency delta of the best candidate against the baseline.
fn latency_improvement_text(outcome: &CdnTestOutcome) -> Option<String> {
    let best = outcome
        .candidates
        .iter()
        .map(|candidate| candidate.tcp_latency_ms)
        .fold(f64::INFINITY, f64::min);
    match (
        best,
        outcome.default_node.as_ref().map(|node| node.tcp_latency_ms),
    ) {
        (active, Some(base)) if active.is_finite() && base > 0.0 => {
            Some(format!("{:+.1}%", (active - base) / base * 100.0))
        }
        _ => None,
    }
}

impl Cdn {
    fn new(ctx: &AppContext) -> Self {
        Self {
            ui_weak: ctx.ui_weak.clone(),
            dispatcher: ctx.dispatcher.clone(),
            event_bus: ctx.event_bus.clone(),
            store: ctx.store.clone(),
            current_settings: ctx.current_settings.clone(),
            candidates: ctx.labs_candidates.clone(),
            toast_queue: ctx.toast_queue.clone(),
        }
    }

    fn lang(&self) -> Language {
        self.store.lock().language()
    }

    /// Point the accelerator at `ip`, persist it and repaint the tab.
    ///
    /// `speed` is `Some` when the IP came from a measured candidate (which also
    /// records the throughput) and `None` for the manual override (which
    /// instead clears the inline validation error).
    async fn apply_ip(self, ip: IpAddr, speed: Option<f64>) {
        let lang = self.lang();
        let Some(service) = self.dispatcher.cdn_service() else {
            return;
        };
        let service = service.clone();

        let settings = self.current_settings.lock().clone();
        if service
            .apply_ip(ip, speed.unwrap_or(0.0), &settings)
            .await
            .is_err()
        {
            return;
        }

        let mut updated = settings.clone();
        updated.cdn_acceleration.active_ip = Some(ip.to_string());
        if let Some(speed) = speed {
            updated.cdn_acceleration.active_speed_mbps = Some(speed);
        }
        if let Ok(saved) = self.dispatcher.save_settings(&updated).await {
            *self.current_settings.lock() = saved;
        }

        push_toast(
            &self.ui_weak,
            &self.toast_queue,
            i18n::format_toast_cdn_applied(&ip.to_string(), lang),
            "success",
            Duration::from_secs(4),
        );

        let candidates = self.candidates.lock().clone();
        let ip_text = ip.to_string();
        let ui_weak = self.ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                let mut form = ui.get_labs_form();
                form.cdn_active_ip = SharedString::from(ip_text.clone());
                match speed {
                    Some(speed) => {
                        form.cdn_active_speed_text =
                            SharedString::from(format!("{speed:.2} MB/s"));
                    }
                    None => form.cdn_manual_ip_error = SharedString::default(),
                }
                form.cdn_status_type = SharedString::from("ready");
                form.cdn_status_label = SharedString::from(i18n::cdn_ready_label(lang));
                ui.set_labs_form(form);
                ui.set_cdn_candidates(cdn_candidates_to_slint(&candidates, &ip_text));
            });
        });
    }

    /// Persist a finished speedtest: active node, candidates and last-run time.
    async fn finish_test(self, outcome: CdnTestOutcome) {
        let now_ms = limedl_core::now_ms();

        if let Ok(mut current) = self.dispatcher.get_settings().await {
            match &outcome.state {
                AccelState::Ready => {
                    current.cdn_acceleration.active_ip = outcome.active_ip.map(|ip| ip.to_string());
                    current.cdn_acceleration.active_speed_mbps = outcome.active_speed_mbps;
                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                    current.cdn_acceleration.last_error = None;
                }
                AccelState::Error(msg) => {
                    current.cdn_acceleration.last_error = Some(msg.clone());
                    current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                }
                _ => {}
            }
            if let Ok(saved) = self.dispatcher.save_settings(&current).await {
                *self.current_settings.lock() = saved;
            }
        }

        *self.candidates.lock() = outcome.candidates.clone();

        let candidates = outcome.candidates.clone();
        let active_ip = outcome
            .active_ip
            .map(|ip| ip.to_string())
            .unwrap_or_default();
        let node_text = default_node_text(&outcome);
        let speed_text = speed_improvement_text(&outcome);
        let latency_text = latency_improvement_text(&outcome);
        let is_ready = matches!(outcome.state, AccelState::Ready);
        let last_test_time = format_timestamp_ms(now_ms);

        let ui_weak = self.ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            with_ui(&ui_weak, |ui| {
                let mut form = ui.get_labs_form();
                if let Some(text) = node_text {
                    form.cdn_default_node_text = SharedString::from(text);
                }
                if let Some(text) = speed_text {
                    form.cdn_speed_improvement_text = SharedString::from(text);
                }
                if let Some(text) = latency_text {
                    form.cdn_latency_improvement_text = SharedString::from(text);
                }
                if is_ready {
                    form.cdn_last_test_time = SharedString::from(last_test_time);
                }
                ui.set_labs_form(form);
                ui.set_cdn_candidates(cdn_candidates_to_slint(&candidates, &active_ip));
            });
        });
    }
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let cdn = Cdn::new(ctx);

    // Start a speedtest: flip the tab into "testing", run it, then repaint.
    {
        let cdn = cdn.clone();
        ui.on_start_cdn_test(move || {
            let Some(settings) = read_ui(&cdn.ui_weak, |ui| {
                let lang = cdn.lang();
                let mut form = ui.get_labs_form();
                form.cdn_is_testing = true;
                form.cdn_status_type = SharedString::from("testing");
                form.cdn_status_label =
                    SharedString::from(i18n::format_cdn_status_label(true, lang));
                form.cdn_phase_label = SharedString::from(i18n::cdn_phase_fetching_label(lang));
                form.cdn_progress_percent = 0.0;
                form.cdn_progress_label = SharedString::from("0 / 0");
                form.cdn_last_error = SharedString::default();
                ui.set_labs_form(form.clone());

                let mut settings = cdn.current_settings.lock().clone();
                update_app_settings_from_labs_form(&mut settings, &form);
                settings
            }) else {
                return;
            };

            let cdn = cdn.clone();
            tokio::spawn(async move {
                let Some(service) = cdn.dispatcher.cdn_service() else {
                    return;
                };
                let service = service.clone();

                match service.start_test(settings).await {
                    Ok(()) => {
                        let outcome = service.monitor_test(cdn.event_bus.clone()).await;
                        cdn.finish_test(outcome).await;
                    }
                    Err(err) => {
                        tracing::error!("启动 CDN 测速失败: {err:#}");
                        let err_msg = err.to_string();
                        let lang = cdn.lang();
                        push_toast(
                            &cdn.ui_weak,
                            &cdn.toast_queue,
                            i18n::format_toast_cdn_test_failed(&err_msg, lang),
                            "error",
                            Duration::from_secs(6),
                        );
                        let ui_weak = cdn.ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            with_ui(&ui_weak, |ui| {
                                let mut form = ui.get_labs_form();
                                form.cdn_is_testing = false;
                                form.cdn_status_type = SharedString::from("error");
                                form.cdn_status_label =
                                    SharedString::from(i18n::cdn_test_failed_label(lang));
                                form.cdn_last_error = SharedString::from(err_msg);
                                ui.set_labs_form(form);
                            });
                        });
                    }
                }
            });
        });
    }

    // Cancel a running speedtest (synchronous: the service just trips a token).
    {
        let cdn = cdn.clone();
        ui.on_cancel_cdn_test(move || {
            let Some(service) = cdn.dispatcher.cdn_service() else {
                return;
            };
            service.cancel_test();
            let lang = cdn.lang();
            with_ui(&cdn.ui_weak, |ui| {
                let mut form = ui.get_labs_form();
                form.cdn_is_testing = false;
                form.cdn_status_type = SharedString::from("idle");
                form.cdn_status_label = SharedString::from(i18n::cdn_cancelled_label(lang));
                ui.set_labs_form(form);
            });
        });
    }

    // Clear the active node and its measured candidates.
    {
        let cdn = cdn.clone();
        ui.on_clear_cdn_test(move || {
            let cdn = cdn.clone();
            tokio::spawn(async move {
                let Some(service) = cdn.dispatcher.cdn_service() else {
                    return;
                };
                service.clear().await;
                cdn.candidates.lock().clear();

                let lang = cdn.lang();
                let mut settings = cdn.current_settings.lock().clone();
                settings.cdn_acceleration.active_ip = None;
                settings.cdn_acceleration.active_speed_mbps = None;
                settings.cdn_acceleration.last_test_at_ms = None;
                settings.cdn_acceleration.last_error = None;
                if let Ok(saved) = cdn.dispatcher.save_settings(&settings).await {
                    *cdn.current_settings.lock() = saved;
                }

                push_toast(
                    &cdn.ui_weak,
                    &cdn.toast_queue,
                    i18n::format_toast_cdn_cleared(lang).to_string(),
                    "info",
                    Duration::from_secs(4),
                );

                let ui_weak = cdn.ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    with_ui(&ui_weak, |ui| {
                        let mut form = ui.get_labs_form();
                        form.cdn_active_ip = SharedString::default();
                        form.cdn_active_speed_text = SharedString::default();
                        form.cdn_status_type = SharedString::from("idle");
                        form.cdn_status_label = SharedString::from(i18n::cdn_idle_label(lang));
                        ui.set_labs_form(form);
                        ui.set_cdn_candidates(cdn_candidates_to_slint(&[], ""));
                    });
                });
            });
        });
    }

    // Switch to a measured candidate.
    {
        let cdn = cdn.clone();
        ui.on_apply_cdn_candidate(move |ip_str, speed_mbps| {
            let Ok(ip) = ip_str.parse::<IpAddr>() else {
                return;
            };
            let cdn = cdn.clone();
            tokio::spawn(async move {
                cdn.apply_ip(ip, Some(speed_mbps as f64)).await;
            });
        });
    }

    // Manual IP override, with inline validation.
    {
        let cdn = cdn.clone();
        ui.on_apply_manual_cdn_ip(move |ip_str| {
            match ip_str.trim().parse::<IpAddr>() {
                Ok(ip) => {
                    let cdn = cdn.clone();
                    tokio::spawn(async move {
                        cdn.apply_ip(ip, None).await;
                    });
                }
                Err(_) => {
                    let lang = cdn.lang();
                    with_ui(&cdn.ui_weak, |ui| {
                        let mut form = ui.get_labs_form();
                        form.cdn_manual_ip_error = SharedString::from(i18n::format_invalid_ip(lang));
                        ui.set_labs_form(form);
                    });
                }
            }
        });
    }

    {
        let ui_weak = ctx.ui_weak.clone();
        ui.on_toggle_cdn_advanced(move || {
            with_ui(&ui_weak, |ui| {
                let mut form = ui.get_labs_form();
                form.cdn_show_advanced = !form.cdn_show_advanced;
                ui.set_labs_form(form);
            });
        });
    }
}
