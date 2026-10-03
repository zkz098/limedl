//! Engine → window: the two paths that change the UI without a user action.
//!
//! `main()` wires both up at startup (`event_stream::start_event_bus_listener`
//! and the `.slint` refresh timer behind `background_refresh`), so nothing else
//! in the suite could see them: the fixture builds the bus, but a listener that
//! reads the wrong serde field, forgets the repaint or drops the selection only
//! shows up as "the window stopped updating".
//!
//! Each scenario starts the real listener on its own window's bus, so the
//! assertions cover the handler wiring (`on_progress`, `on_updated`, …) and not
//! just the store mutation behind it.

use limedl_core::event_bus::DownloadEvent;
use limedl_core::types::{DownloadProgress, DownloadState};

use slint::Model;

use super::super::{Language, TestUi, http_task, http_wire, new_window};

/// The row's progress bar, as the list model reports it (0.0 – 1.0).
fn row_progress(ui: &TestUi, row: usize) -> f32 {
    ui.window
        .get_tasks()
        .row_data(row)
        .expect("the row exists")
        .progress
}

/// The row's speed cell, as the list model reports it.
fn row_speed(ui: &TestUi, row: usize) -> String {
    ui.window
        .get_tasks()
        .row_data(row)
        .expect("the row exists")
        .speed_text
        .to_string()
}

/// Start the subscriber the way `main()` does.
fn start_listener(ui: &TestUi) {
    crate::event_stream::start_event_bus_listener(&ui.ctx, ui.ctx.event_bus.subscribe());
}

/// Progress ticks are the only event that fires ~3×/s during a download; they
/// repaint the row they name and must leave every other row alone.
pub(super) async fn a_progress_tick_repaints_only_the_row_it_names() {
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
    ]);
    start_listener(&ui);

    assert!((row_progress(&ui, 0) - 0.1).abs() < 1e-6, "seeded at 10%");

    ui.ctx.event_bus.publish(DownloadEvent::Progress {
        progress: DownloadProgress {
            id: http_wire(1),
            state: DownloadState::Downloading,
            downloaded_bytes: 55,
            total_bytes: Some(100),
            speed_bytes_per_second: Some(1_048_576.0),
            eta_seconds: Some(3),
            connection_count: 2,
            allocated_thread_count: None,
            error: None,
            uploaded_bytes: None,
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None,
            degraded: false,
            disk_type: None,
            flushing: false,
        },
    });
    ui.pump_until("the tick to reach the row", || row_progress(&ui, 0) > 0.5)
        .await;

    assert!(
        (row_progress(&ui, 0) - 0.55).abs() < 1e-6,
        "the tick's bytes must reach the progress bar"
    );
    assert_eq!(
        row_speed(&ui, 0),
        crate::bridge::format_speed(Some(1_048_576.0)),
        "…and its speed the speed cell"
    );
    assert!(
        (row_progress(&ui, 1) - 0.1).abs() < 1e-6,
        "the task the event did not name must not move"
    );
    assert!(
        ui.core.calls().is_empty(),
        "a progress tick is a pure repaint: it must not ask the engine for anything"
    );
}

/// A terminal state change repaints the row, refreshes the inspector when it is
/// showing that task, and raises the completion toast.
pub(super) async fn a_terminal_state_event_toasts_and_repaints_the_row() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    start_listener(&ui);

    // Open the inspector on the task first: the event has to re-render it, which
    // is the difference between "the row updated" and "the detail panel is stale".
    ui.window.invoke_open_inspector(http_wire(1).into());
    assert!(ui.window.get_show_inspector());

    let finished = http_task(1, "alpha-renamed.bin", DownloadState::Completed, 100, 100);
    ui.ctx.event_bus.publish(DownloadEvent::Updated {
        summary: Box::new(finished),
    });
    ui.pump_until("the update to reach the row", || {
        row_progress(&ui, 0) > 0.99
    })
    .await;

    assert_eq!(
        ui.window
            .get_tasks()
            .row_data(0)
            .expect("the row exists")
            .state_code
            .as_str(),
        "completed",
        "the row follows the summary the event carried"
    );
    assert_eq!(
        ui.window.get_inspector_info().file_name.as_str(),
        "alpha-renamed.bin",
        "an open inspector must re-render the task the event changed"
    );
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_state(
            "alpha-renamed.bin",
            &DownloadState::Completed,
            Language::ZhCn,
        )
        .as_str(),
    );
}

/// An event for a task the backend no longer knows drops the row and closes the
/// inspector: the task was removed elsewhere (tray, RPC client), and the window
/// must not keep showing a phantom.
pub(super) async fn an_event_for_a_task_the_backend_forgot_drops_the_row_and_closes_the_inspector()
{
    let ui = new_window();
    ui.seed(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Downloading, 10, 100),
    ]);
    start_listener(&ui);
    ui.window.invoke_open_inspector(http_wire(1).into());
    assert!(ui.window.get_show_inspector());

    // The backend no longer knows task 1: it is not in `list()`/`status()`
    // anymore, only the UI still shows it.
    ui.core.set_tasks(vec![http_task(
        2,
        "bravo.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);

    let stale = http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100);
    ui.ctx
        .event_bus
        .publish(DownloadEvent::Updated {
            summary: Box::new(stale),
        });
    ui.pump_until("the row to be dropped", || ui.visible_rows() == 1)
        .await;

    assert_eq!(
        ui.window
            .get_tasks()
            .row_data(0)
            .expect("the other row survives")
            .file_name
            .as_str(),
        "bravo.bin"
    );
    assert!(
        !ui.window.get_show_inspector(),
        "the inspector cannot keep showing a task that is gone"
    );
    assert_eq!(
        ui.core.statuses(),
        vec![http_wire(1)],
        "the listener asks the backend about the task before dropping it"
    );
    assert!(
        ui.core.removes().is_empty(),
        "the row went away because the event said so, not because the UI removed it"
    );
}

/// A subscriber that lagged gets the whole engine list back and has to replace
/// what it shows, not append to it.
///
/// The lag is forced for real: the fixture's bus has capacity 64, the listener
/// task cannot run until the test yields, and the backend list changes between
/// the seed and the flood. Recovery is therefore only correct if the listener
/// asks the engine (`list()`) rather than replaying events.
pub(super) async fn a_lagged_subscriber_gets_the_whole_list_back() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    start_listener(&ui);

    // The engine now reports a different list than the UI store holds.
    ui.core.set_tasks(vec![
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Completed, 100, 100),
    ]);

    // Publish past the capacity (64) without awaiting: the listener task has not
    // been polled yet, so its next `recv()` must return `Lagged`.
    for _ in 0..100 {
        ui.ctx
            .event_bus
            .publish(DownloadEvent::Warning {
                id: http_wire(1),
                message: "flood".into(),
            });
    }

    ui.pump_until("the list to be replaced", || ui.visible_rows() == 2)
        .await;

    let names: Vec<String> = (0..ui.visible_rows())
        .filter_map(|row| ui.window.get_tasks().row_data(row))
        .map(|item| item.file_name.to_string())
        .collect();
    assert_eq!(
        names,
        vec!["bravo.bin", "charlie.bin"],
        "the recovery payload replaces the list (and drops what it left out)"
    );
    assert_eq!(
        ui.window.get_count_paused().as_str(),
        "1",
        "the category counters follow the recovered list"
    );
    assert_eq!(ui.window.get_count_completed().as_str(), "1");
}

/// A core warning is a toast; anti-leech bans arrive one per peer, so the
/// duplicate window has to collapse them.
pub(super) async fn a_warning_becomes_a_toast_and_duplicates_are_collapsed() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);
    start_listener(&ui);

    for _ in 0..2 {
        ui.ctx.event_bus.publish(DownloadEvent::Warning {
            id: http_wire(1),
            message: "peer banned".into(),
        });
    }
    ui.ctx.event_bus.publish(DownloadEvent::Warning {
        id: http_wire(1),
        message: "disk full".into(),
    });
    ui.pump_until("both warnings to be delivered", || ui.toasts().len() == 2)
        .await;

    let toasts = ui.toasts();
    assert_eq!(
        toasts.len(),
        2,
        "the repeated ban is collapsed, the different message is not: {toasts:?}"
    );
    assert!(toasts.iter().all(|(kind, _)| kind == "warning"));
    assert_eq!(
        toasts[0].1,
        crate::i18n::format_warning_with_file("alpha.bin", "peer banned"),
        "the toast names the file the warning belongs to"
    );
}

/// The CDN tab has no other live feedback: the speedtest's phase, progress and
/// outcome all arrive as events.
pub(super) async fn the_cdn_speedtest_reports_its_progress_and_outcome() {
    let ui = new_window();
    start_listener(&ui);
    ui.window.invoke_open_labs();

    ui.ctx.event_bus.publish(DownloadEvent::CdnProgress {
        phase: "latency".into(),
        current: 3,
        total: 6,
    });
    // The predicate is the *event's* progress label: an idle labs form already
    // carries a non-zero percentage (`refresh_labs_state` pushes 100.0 when no
    // test is running), so a percentage-based wait would pass before the event
    // landed.
    ui.pump_until("the CDN progress to reach the labs form", || {
        ui.window.get_labs_form().cdn_progress_label.as_str() == "3 / 6"
    })
    .await;
    {
        let form = ui.window.get_labs_form();
        assert!(form.cdn_is_testing, "progress means a test is running");
        assert_eq!(form.cdn_status_type.as_str(), "testing");
        assert_eq!(
            form.cdn_phase_label.as_str(),
            crate::i18n::cdn_phase_label("latency", Language::ZhCn)
        );
        assert!((form.cdn_progress_percent - 50.0).abs() < 1e-3);
        assert_eq!(form.cdn_progress_label.as_str(), "3 / 6");
    }

    ui.ctx.event_bus.publish(DownloadEvent::CdnComplete {
        state: "Ready".into(),
        active_ip: Some("203.0.113.7".into()),
        active_speed_mbps: Some(42.5),
    });
    ui.pump_until("the CDN outcome to reach the labs form", || {
        !ui.window.get_labs_form().cdn_is_testing
    })
    .await;
    {
        let form = ui.window.get_labs_form();
        assert_eq!(form.cdn_status_type.as_str(), "ready");
        assert_eq!(
            form.cdn_status_label.as_str(),
            crate::i18n::cdn_ready_label(Language::ZhCn)
        );
        assert_eq!(form.cdn_active_ip.as_str(), "203.0.113.7");
        assert_eq!(form.cdn_active_speed_text.as_str(), "42.50 MB/s");
        assert!(form.cdn_last_error.is_empty(), "a success clears the error");
    }
    ui.assert_toast(
        "success",
        crate::i18n::format_toast_cdn_test_done(Some("203.0.113.7"), Language::ZhCn).as_str(),
    );
}

/// The periodic resync (the `.slint` timer behind `background_refresh`) re-reads
/// the list from the backend — the only way a task started by another client
/// shows up while this window is open.
pub(super) async fn a_background_refresh_resyncs_the_whole_list() {
    let ui = new_window();
    ui.seed(vec![http_task(
        1,
        "alpha.bin",
        DownloadState::Downloading,
        10,
        100,
    )]);

    // The backend answers with a list the store has never seen, which is exactly
    // the "another client added a task" case.
    ui.core.set_tasks(vec![
        http_task(1, "alpha.bin", DownloadState::Downloading, 10, 100),
        http_task(2, "bravo.bin", DownloadState::Paused, 20, 100),
        http_task(3, "charlie.bin", DownloadState::Completed, 100, 100),
    ]);
    assert_eq!(ui.visible_rows(), 1, "the window starts on the stale list");

    ui.window.invoke_background_refresh();
    ui.pump_until("the list to be resynced", || ui.visible_rows() == 3)
        .await;

    assert_eq!(ui.window.get_count_all().as_str(), "3");
    assert_eq!(ui.window.get_count_completed().as_str(), "1");
}
